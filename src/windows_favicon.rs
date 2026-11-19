use base64::{Engine, engine::general_purpose::STANDARD};
use std::rc::Rc;
use webview2_com::{
    FaviconChangedEventHandler, GetFaviconCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    take_pwstr,
};
use windows::core::{Interface, PWSTR};
use wry::{WebView, WebViewExtWindows};

pub struct FaviconHook {
    core: ICoreWebView2_15,
    token: i64,
}
impl Drop for FaviconHook {
    fn drop(&mut self) {
        let _ = unsafe { self.core.remove_FaviconChanged(self.token) };
    }
}

/// Use WebView2's decoded icon, without making extra toolbar network requests.
pub fn install(
    page: &WebView,
    notify: impl Fn(String, String) + 'static,
) -> windows::core::Result<FaviconHook> {
    let core = page.webview().cast::<ICoreWebView2_15>()?;
    let notify = Rc::new(notify);
    let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
        let Some(sender) = sender else {
            return Ok(());
        };
        unsafe {
            let mut raw = PWSTR::null();
            sender.Source(&mut raw)?;
            let source = take_pwstr(raw);
            let notify = Rc::clone(&notify);
            sender.cast::<ICoreWebView2_15>()?.GetFavicon(
                COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
                &GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                    result?;
                    let Some(stream) = stream else {
                        return Ok(());
                    };
                    // Bound memory and IPC size even when the page provides a huge icon.
                    let mut bytes = Vec::new();
                    loop {
                        let mut chunk = [0u8; 4096];
                        let mut count = 0;
                        stream
                            .Read(
                                chunk.as_mut_ptr().cast(),
                                chunk.len() as u32,
                                Some(&mut count),
                            )
                            .ok()?;
                        if count == 0 {
                            break;
                        }
                        if bytes.len() + count as usize > 256 * 1024 {
                            return Ok(());
                        }
                        bytes.extend_from_slice(&chunk[..count as usize]);
                    }
                    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                        notify(
                            source,
                            format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
                        );
                    }
                    Ok(())
                })),
            )?;
        }
        Ok(())
    }));
    let mut hook = FaviconHook { core, token: 0 };
    unsafe {
        hook.core.add_FaviconChanged(&handler, &mut hook.token)?;
    }
    Ok(hook)
}
