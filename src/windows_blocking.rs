use crate::blocking::AdBlocker;
use std::{cell::RefCell, rc::Rc};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler, take_pwstr,
};
use windows::{
    Win32::System::Com::IStream,
    core::{Interface, PWSTR, w},
};
use wry::{WebView, WebViewExtWindows};

pub struct RequestHook {
    core: ICoreWebView2,
    token: i64,
}

impl Drop for RequestHook {
    fn drop(&mut self) {
        // The hook and its callbacks live on WebView2's UI thread.
        let _ = unsafe { self.core.remove_WebResourceRequested(self.token) };
    }
}

fn request_kind(context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT, is_top: bool) -> &'static str {
    match context {
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT if is_top => "document",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => "subdocument",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
        | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => "xmlhttprequest",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
        _ => "other",
    }
}

/// Install before navigating to remote content. No response body is downloaded for blocked requests.
pub fn install(
    page: &WebView,
    blocker: Rc<AdBlocker>,
    top_url: Rc<RefCell<String>>,
    notify: impl Fn(Result<u64, String>) + 'static,
) -> windows::core::Result<RequestHook> {
    let core = page.webview();
    let environment = page.environment();
    let core22 = core.cast::<ICoreWebView2_22>()?;
    let mut token = 0;
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let result = (|| -> windows::core::Result<()> {
            unsafe {
                let request = args.Request()?;
                let mut raw_url = PWSTR::null();
                request.Uri(&mut raw_url)?;
                let url = take_pwstr(raw_url);
                let mut raw_method = PWSTR::null();
                request.Method(&mut raw_method)?;
                let method = take_pwstr(raw_method);
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                args.ResourceContext(&mut context)?;
                let source = top_url.borrow();
                let is_top = url.split('#').next() == source.split('#').next();
                if blocker.should_block(&url, &source, request_kind(context, is_top), &method) {
                    let response = environment.CreateWebResourceResponse(
                        None::<&IStream>,
                        403,
                        w!("Blocked by Plain Browser"),
                        w!("Cache-Control: no-store\r\nContent-Length: 0\r\n"),
                    )?;
                    args.SetResponse(&response)?;
                    blocker.record_block();
                    notify(Ok(blocker.blocked()));
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            notify(Err(format!("광고 요청 검사 오류: {error}")));
        }
        result
    }));
    unsafe {
        core.add_WebResourceRequested(&handler, &mut token)?;
    }
    let hook = RequestHook { core, token };
    // Include frame, shared-worker and service-worker requests; do not rely on JS monkey patches.
    unsafe {
        core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
            w!("http://*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
        )?;
        core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
            w!("https://*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
        )?;
    }
    Ok(hook)
}
