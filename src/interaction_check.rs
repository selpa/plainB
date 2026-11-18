use super::*;
use std::time::{Duration, Instant};
use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
use windows::core::HSTRING;
use wry::WebViewExtWindows;

pub(super) struct InteractionCheck {
    step: u8,
    first: TabId,
    second: TabId,
    third: TabId,
    received: Option<String>,
    start: Instant,
    failure: Option<String>,
}
impl InteractionCheck {
    pub fn new() -> Self {
        Self {
            step: 0,
            first: 0,
            second: 0,
            third: 0,
            received: None,
            start: Instant::now(),
            failure: None,
        }
    }
    pub fn receive(&mut self, json: String) {
        self.received = Some(json);
    }
    pub fn finished(&self) -> bool {
        self.step == 30 || self.failure.is_some()
    }
    pub fn result(self) -> AppResult<()> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        if self.step != 30 {
            return Err("interaction check incomplete".into());
        }
        println!(
            "PASS: native hover, links, history, reorder; square buttons; popup block, allow once, persistent permission, automatic allow, revoke and dismiss."
        );
        Ok(())
    }
    fn fail(&mut self, message: &str) {
        self.failure = Some(format!("step {}: {message}", self.step));
    }
    fn protocol(page: &WebView, method: &str, parameters: serde_json::Value) {
        unsafe {
            page.webview()
                .CallDevToolsProtocolMethod(
                    &HSTRING::from(method),
                    &HSTRING::from(parameters.to_string()),
                    &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(()))),
                )
                .expect("native fixture input");
        }
    }
    fn click(page: &WebView, id: &str) {
        Self::protocol(
            page,
            "Runtime.evaluate",
            serde_json::json!({"expression":format!("document.getElementById({}).click()",serde_json::to_string(id).unwrap()),"userGesture":true}),
        );
    }
    fn evaluate(browser: &Browser, page: &WebView, script: &str) {
        let proxy = browser.proxy.clone();
        page.evaluate_script_with_callback(script, move |json| {
            let _ = proxy.send_event(BrowserEvent::CheckResult(json));
        })
        .unwrap();
    }
    fn toolbar(browser: &Browser, script: &str) {
        browser.toolbar.evaluate_script(script).unwrap();
    }
    fn navigate(browser: &Browser, url: &str) {
        Self::toolbar(
            browser,
            &format!(
                "document.getElementById('address').value={};document.getElementById('navigation').dispatchEvent(new Event('submit',{{cancelable:true}}))",
                serde_json::to_string(url).unwrap()
            ),
        );
    }
    fn drag(browser: &Browser, id: TabId, to_end: bool) {
        // Exercise the actual pointer handlers and IPC against the real toolbar DOM.
        Self::toolbar(
            browser,
            &format!(
                r#"(()=>{{
          const list=document.getElementById('tabs'), from=list.querySelector('.tab-title[data-tab-id="{id}"]');
          const titles=[...list.querySelectorAll('.tab-title')], target={target};
          const a=from.getBoundingClientRect(),b=target.getBoundingClientRect(),y=a.top+a.height/2;
          const x={x};
          from.dispatchEvent(new PointerEvent('pointerdown',{{bubbles:true,pointerId:1,pointerType:'mouse',button:0,buttons:1,clientX:a.left+a.width/2,clientY:y}}));
          list.dispatchEvent(new PointerEvent('pointermove',{{bubbles:true,cancelable:true,pointerId:1,pointerType:'mouse',buttons:1,clientX:x,clientY:y}}));
          list.dispatchEvent(new PointerEvent('pointerup',{{bubbles:true,pointerId:1,pointerType:'mouse',button:0,clientX:x,clientY:y}}));
        }})()"#,
                target = if to_end {
                    "titles[titles.length-1]"
                } else {
                    "titles[0]"
                },
                x = if to_end { "b.right+50" } else { "b.left+1" }
            ),
        );
    }
    pub fn advance(&mut self, ready: bool, browser: &Browser) {
        if self.finished() {
            return;
        }
        if self.start.elapsed() > Duration::from_secs(45) {
            self.fail("native interaction check timed out");
            return;
        }
        if !ready {
            return;
        }
        let Some(active) = browser.tabs.active() else {
            return;
        };
        match self.step {
            0 if !active.loading => {
                self.first = browser.tabs.active_id().unwrap();
                Self::navigate(browser, "http://plain-check.localhost/a");
                self.step = 1;
            }
            1 if !active.loading && active.url.ends_with("/a") => {
                Self::evaluate(
                    browser,
                    &active.view,
                    "(()=>{const r=document.getElementById('same').getBoundingClientRect();return {x:r.left+r.width/2,y:r.top+r.height/2}})()",
                );
                self.step = 2;
            }
            2 => {
                if let Some(json) = self.received.take() {
                    let point: serde_json::Value = serde_json::from_str(&json).unwrap();
                    Self::protocol(
                        &active.view,
                        "Input.dispatchMouseEvent",
                        serde_json::json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
                    );
                    self.step = 3;
                }
            }
            3 if active.hovered_link.ends_with("/same") => {
                Self::evaluate(
                    browser,
                    &browser.toolbar,
                    "document.getElementById('status').textContent",
                );
                self.step = 4;
            }
            4 => {
                if let Some(json) = self.received.take() {
                    if !json.contains("plain-check.localhost/same") {
                        self.fail("hover URL missing from footer");
                        return;
                    }
                    Self::protocol(
                        &active.view,
                        "Input.dispatchMouseEvent",
                        serde_json::json!({"type":"mouseMoved","x":1,"y":1}),
                    );
                    self.step = 5;
                }
            }
            5 if active.hovered_link.is_empty() => {
                Self::click(&active.view, "same");
                self.step = 6;
            }
            6 if !active.loading && active.url.ends_with("/same") => {
                if browser.tabs.len() != 1 || browser.tabs.active_id() != Some(self.first) {
                    self.fail("same-site link opened another tab");
                    return;
                }
                Self::click(&active.view, "external");
                self.step = 7;
            }
            7 if browser.tabs.len() == 2
                && !active.loading
                && active.url.ends_with("/external") =>
            {
                self.second = browser.tabs.active_id().unwrap();
                if self.second == self.first
                    || !browser.tabs.get(self.first).unwrap().url.ends_with("/same")
                {
                    self.fail("cross-site link replaced source page");
                    return;
                }
                Self::navigate(browser, "http://plain-check.localhost/direct");
                self.step = 8;
            }
            8 if !active.loading && active.url.ends_with("/direct") => {
                if browser.tabs.len() != 2 || browser.tabs.active_id() != Some(self.second) {
                    self.fail("typed address opened a tab");
                    return;
                }
                Self::toolbar(browser, "document.getElementById('back').click()");
                self.step = 9;
            }
            9 if !active.loading && active.url.ends_with("/external") => {
                if browser.tabs.len() != 2 {
                    self.fail("history opened another tab");
                    return;
                }
                Self::toolbar(browser, "document.getElementById('forward').click()");
                self.step = 10;
            }
            10 if !active.loading && active.url.ends_with("/direct") => {
                Self::click(&active.view, "blank");
                self.step = 11;
            }
            11 if browser.tabs.len() == 3 && !active.loading && active.url.ends_with("/blank") => {
                self.third = browser.tabs.active_id().unwrap();
                Self::drag(browser, self.third, false);
                self.step = 12;
            }
            12 if browser.tabs.iter().next().map(|(id, _)| id) == Some(self.third) => {
                if browser.tabs.active_id() != Some(self.third)
                    || !browser.tabs.get(self.first).unwrap().url.ends_with("/same")
                    || !browser
                        .tabs
                        .get(self.second)
                        .unwrap()
                        .url
                        .ends_with("/direct")
                {
                    self.fail("reorder changed selected page or source state");
                    return;
                }
                Self::evaluate(
                    browser,
                    &browser.toolbar,
                    "[...document.querySelectorAll('.tab')].map(tab=>Number(tab.dataset.tabId))",
                );
                self.step = 13;
            }
            13 => {
                if let Some(json) = self.received.take() {
                    let order: Vec<TabId> = serde_json::from_str(&json).unwrap_or_default();
                    if order != [self.third, self.first, self.second] {
                        self.fail("visual tab order not synchronized");
                        return;
                    }
                    Self::drag(browser, self.third, true);
                    self.step = 14;
                }
            }
            14 if browser.tabs.iter().map(|(id, _)| id).collect::<Vec<_>>()
                == [self.first, self.second, self.third] =>
            {
                if browser.tabs.active_id() != Some(self.third) {
                    self.fail("reordering selected a different tab");
                    return;
                }
                let _ = browser
                    .proxy
                    .send_event(BrowserEvent::Command(Command::PopupRevoke {
                        id: self.third,
                    }));
                self.step = 15;
            }
            15 if !browser.popup_permissions.allows("plain-check.other.test") => {
                Self::protocol(
                    &active.view,
                    "Runtime.evaluate",
                    serde_json::json!({"expression":"window.open('/popup-once')", "userGesture":false}),
                );
                self.step = 16;
            }
            16 if active.pending_popup.is_some() => {
                if browser.tabs.len() != 3 {
                    self.fail("blocked popup created a tab");
                    return;
                }
                Self::toolbar(browser, "document.getElementById('popups').click()");
                self.step = 17;
            }
            17 if browser.popup_menu => {
                Self::evaluate(
                    browser,
                    &browser.toolbar,
                    "!document.getElementById('popup-panel').hidden && ['back','forward','reload','popups'].every(id=>{const r=document.getElementById(id).getBoundingClientRect();return r.width===38 && r.height===38}) && (()=>{const r=document.querySelector('.go').getBoundingClientRect();return r.width===38 && r.height===38})()",
                );
                self.step = 18;
            }
            18 => {
                if let Some(json) = self.received.take() {
                    if json != "true" {
                        self.fail("popup panel or square button geometry incorrect");
                        return;
                    }
                    Self::toolbar(browser, "document.getElementById('popup-once').click()");
                    self.step = 19;
                }
            }
            19 if browser.tabs.len() == 4
                && !active.loading
                && active.url.ends_with("/popup-once") =>
            {
                if browser.popup_permissions.allows("plain-check.other.test") {
                    self.fail("allow once persisted permission");
                    return;
                }
                let _ = browser
                    .proxy
                    .send_event(BrowserEvent::Command(Command::SelectTab { id: self.third }));
                self.step = 20;
            }
            20 if browser.tabs.active_id() == Some(self.third) => {
                Self::protocol(
                    &active.view,
                    "Runtime.evaluate",
                    serde_json::json!({"expression":"window.open('/popup-always')", "userGesture":false}),
                );
                self.step = 21;
            }
            21 if active.pending_popup.is_some() => {
                Self::toolbar(
                    browser,
                    "document.getElementById('popups').click();document.getElementById('popup-permission').click()",
                );
                self.step = 22;
            }
            22 if browser.tabs.len() == 5
                && !active.loading
                && active.url.ends_with("/popup-always") =>
            {
                if !browser.popup_permissions.allows("plain-check.other.test") {
                    self.fail("always allow not saved");
                    return;
                }
                let _ = browser
                    .proxy
                    .send_event(BrowserEvent::Command(Command::SelectTab { id: self.third }));
                self.step = 23;
            }
            23 if browser.tabs.active_id() == Some(self.third) => {
                Self::protocol(
                    &active.view,
                    "Runtime.evaluate",
                    serde_json::json!({"expression":"window.open('/popup-auto')", "userGesture":false}),
                );
                self.step = 24;
            }
            24 if browser.tabs.len() == 6
                && !active.loading
                && active.url.ends_with("/popup-auto") =>
            {
                let _ = browser
                    .proxy
                    .send_event(BrowserEvent::Command(Command::SelectTab { id: self.third }));
                self.step = 25;
            }
            25 if browser.tabs.active_id() == Some(self.third) => {
                Self::toolbar(
                    browser,
                    "document.getElementById('popups').click();document.getElementById('popup-permission').click()",
                );
                self.step = 26;
            }
            26 if !browser.popup_permissions.allows("plain-check.other.test") => {
                Self::protocol(
                    &active.view,
                    "Runtime.evaluate",
                    serde_json::json!({"expression":"window.open('/popup-denied')", "userGesture":false}),
                );
                self.step = 27;
            }
            27 if active.pending_popup.is_some() => {
                if browser.tabs.len() != 6 {
                    self.fail("revoked permission still allowed popup");
                    return;
                }
                Self::toolbar(
                    browser,
                    "document.getElementById('popups').click();document.getElementById('popup-dismiss').click()",
                );
                self.step = 28;
            }
            28 if active.pending_popup.is_none() && !browser.popup_menu => {
                self.step = 30;
            }
            _ => {}
        }
    }
}
