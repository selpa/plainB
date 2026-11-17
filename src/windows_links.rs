use webview2_com::{
    Microsoft::Web::WebView2::Win32::ICoreWebView2, NewWindowRequestedEventHandler, take_pwstr,
};
use windows::core::{BOOL, PWSTR};
use wry::{WebView, WebViewExtWindows};

pub struct NewWindowRequest {
    pub url: String,
    pub user_initiated: bool,
}

pub struct LinkHook {
    core: ICoreWebView2,
    token: i64,
}

impl Drop for LinkHook {
    fn drop(&mut self) {
        let _ = unsafe { self.core.remove_NewWindowRequested(self.token) };
    }
}

/// Suppress the native popup; let the app navigate after this callback returns.
/// Wry's generic new-window callback does not expose IsUserInitiated on Windows.
pub fn install(
    page: &WebView,
    notify: impl Fn(NewWindowRequest) + 'static,
) -> windows::core::Result<LinkHook> {
    let core = page.webview();
    let mut token = 0;
    let handler = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            args.SetHandled(true)?;
            let mut user_initiated = BOOL::default();
            args.IsUserInitiated(&mut user_initiated)?;
            let mut raw_url = PWSTR::null();
            args.Uri(&mut raw_url)?;
            notify(NewWindowRequest {
                url: take_pwstr(raw_url),
                user_initiated: user_initiated.as_bool(),
            });
        }
        Ok(())
    }));
    unsafe {
        core.add_NewWindowRequested(&handler, &mut token)?;
    }
    Ok(LinkHook { core, token })
}
