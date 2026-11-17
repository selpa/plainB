use crate::address;
use std::{cell::RefCell, rc::Rc};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::*, NavigationStartingEventHandler,
    StatusBarTextChangedEventHandler, take_pwstr,
};
use windows::core::{BOOL, Interface, PWSTR};
use wry::{WebView, WebViewExtWindows};

#[derive(Default)]
pub struct NavigationIntent {
    target: RefCell<Option<String>>,
}
impl NavigationIntent {
    pub fn navigate(&self, page: &WebView, url: &str) -> wry::Result<()> {
        self.target.replace(Some(url.to_owned()));
        let result = page.load_url(url);
        if result.is_err() {
            self.target.take();
        }
        result
    }
}

pub enum NavigationEvent {
    Starting(String),
    NewTab(String),
    Hover(String),
    Notice(String),
}
pub struct NavigationHook {
    core: ICoreWebView2,
    status: ICoreWebView2_12,
    starting_token: i64,
    status_token: i64,
}
impl Drop for NavigationHook {
    fn drop(&mut self) {
        unsafe {
            let _ = self.core.remove_NavigationStarting(self.starting_token);
            let _ = self.status.remove_StatusBarTextChanged(self.status_token);
        }
    }
}

pub fn install(
    page: &WebView,
    source: Rc<RefCell<String>>,
    intent: Rc<NavigationIntent>,
    notify: impl Fn(NavigationEvent) + 'static,
) -> windows::core::Result<NavigationHook> {
    let core = page.webview();
    let status = core.cast::<ICoreWebView2_12>()?;
    let notify = Rc::new(notify);
    let navigation = Rc::clone(&notify);
    let handler = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let mut raw = PWSTR::null();
            args.Uri(&mut raw)?;
            let url = take_pwstr(raw);
            if !address::allowed_navigation(&url) {
                args.SetCancel(true)?;
                navigation(NavigationEvent::Notice(
                    "이 버전은 http·https 웹사이트만 엽니다.".into(),
                ));
                return Ok(());
            }
            let mut user = BOOL::default();
            let mut redirected = BOOL::default();
            args.IsUserInitiated(&mut user)?;
            args.IsRedirected(&mut redirected)?;
            let mut kind = COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT;
            if let Ok(args3) = args.cast::<ICoreWebView2NavigationStartingEventArgs3>() {
                args3.NavigationKind(&mut kind)?;
            }
            let explicit =
                !redirected.as_bool() && intent.target.borrow().as_deref() == Some(url.as_str());
            if explicit {
                intent.target.take();
            }
            if user.as_bool()
                && !redirected.as_bool()
                && !explicit
                && kind == COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT
                && address::different_site(&source.borrow(), &url)
            {
                args.SetCancel(true)?;
                navigation(NavigationEvent::NewTab(url));
            } else {
                source.replace(url.clone());
                navigation(NavigationEvent::Starting(url));
            }
        }
        Ok(())
    }));
    let status_handler = StatusBarTextChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            unsafe {
                let mut raw = PWSTR::null();
                sender.cast::<ICoreWebView2_12>()?.StatusBarText(&mut raw)?;
                notify(NavigationEvent::Hover(take_pwstr(raw)));
            }
        }
        Ok(())
    }));
    let mut hook = NavigationHook {
        core,
        status,
        starting_token: 0,
        status_token: 0,
    };
    unsafe {
        hook.core
            .add_NavigationStarting(&handler, &mut hook.starting_token)?;
        hook.status
            .add_StatusBarTextChanged(&status_handler, &mut hook.status_token)?;
    }
    Ok(hook)
}
