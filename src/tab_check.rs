use super::*;
use std::time::{Duration, Instant};

pub(super) struct TabCheck {
    step: u8,
    first: TabId,
    second: TabId,
    received: Option<String>,
    started: Instant,
    failure: Option<String>,
}
impl TabCheck {
    pub fn new() -> Self {
        Self {
            step: 0,
            first: 0,
            second: 0,
            received: None,
            started: Instant::now(),
            failure: None,
        }
    }
    pub fn receive(&mut self, json: String) {
        self.received = Some(json);
    }
    pub fn finished(&self) -> bool {
        self.step == 20 || self.failure.is_some()
    }
    pub fn result(self) -> AppResult<()> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        if self.step != 20 {
            return Err("tab check incomplete".into());
        }
        println!(
            "PASS: real toolbar IPC creates/switches/closes tabs; DOM and history preserved; last-tab replacement and resize verified."
        );
        Ok(())
    }
    fn send(browser: &Browser, script: &str) {
        browser
            .toolbar
            .evaluate_script(script)
            .expect("test toolbar action");
    }
    fn navigate(browser: &Browser, path: &str) {
        Self::send(
            browser,
            &format!(
                "document.getElementById('address').value='http://plain-check.localhost/{path}';document.getElementById('navigation').dispatchEvent(new Event('submit',{{cancelable:true}}))"
            ),
        );
    }
    fn click(browser: &Browser, id: TabId, close: bool) {
        Self::send(
            browser,
            &format!(
                "document.querySelector('.{}[data-tab-id=\"{id}\"]').click()",
                if close { "tab-close" } else { "tab-title" }
            ),
        );
    }
    pub fn advance(&mut self, ready: bool, browser: &Browser) {
        if self.finished() {
            return;
        }
        if self.started.elapsed() > Duration::from_secs(45) {
            self.failure = Some(format!("tab check timed out at step {}", self.step));
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
                Self::navigate(browser, "a");
                self.step = 1;
            }
            1 if !active.loading && active.url.ends_with("/a") => {
                active
                    .view
                    .evaluate_script("document.getElementById('note').value='kept-a'")
                    .unwrap();
                Self::send(browser, "document.getElementById('new-tab').click()");
                self.step = 2;
            }
            2 if browser.tabs.len() == 2 && !active.loading => {
                self.second = browser.tabs.active_id().unwrap();
                Self::navigate(browser, "b");
                self.step = 3;
            }
            3 if !active.loading && active.url.ends_with("/b") => {
                Self::navigate(browser, "c");
                self.step = 4;
            }
            4 if !active.loading && active.url.ends_with("/c") => {
                Self::click(browser, self.first, false);
                self.step = 5;
            }
            5 if browser.tabs.active_id() == Some(self.first) => {
                let second = browser.tabs.get(self.second).unwrap();
                if !active.url.ends_with("/a")
                    || !second.url.ends_with("/c")
                    || !second.view.can_go_back().unwrap_or(false)
                {
                    self.failure = Some("tab addresses/history mixed".into());
                    return;
                }
                let proxy = browser.proxy.clone();
                active
                    .view
                    .evaluate_script_with_callback(
                        "document.getElementById('note').value",
                        move |json| {
                            let _ = proxy.send_event(BrowserEvent::CheckResult(json));
                        },
                    )
                    .unwrap();
                self.step = 6;
            }
            6 => {
                if let Some(json) = self.received.take() {
                    if serde_json::from_str::<String>(&json).ok().as_deref() != Some("kept-a") {
                        self.failure = Some(format!("DOM state lost: {json}"));
                        return;
                    }
                    Self::click(browser, self.second, false);
                    self.step = 7;
                }
            }
            7 if browser.tabs.active_id() == Some(self.second) => {
                Self::send(browser, "document.getElementById('back').click()");
                self.step = 8;
            }
            8 if !active.loading && active.url.ends_with("/b") => {
                Self::click(browser, self.first, true);
                self.step = 9;
            }
            9 if browser.tabs.len() == 1 => {
                if browser.tabs.active_id() != Some(self.second)
                    || browser.tabs.get(self.first).is_some()
                {
                    self.failure = Some("background close changed active tab".into());
                    return;
                }
                Self::click(browser, self.second, true);
                self.step = 10;
            }
            10 if browser.tabs.len() == 1
                && browser.tabs.active_id() != Some(self.second)
                && !active.loading =>
            {
                if active.url != HOME_URL {
                    self.failure = Some("last tab not replaced by home".into());
                    return;
                }
                browser
                    .window
                    .set_inner_size(LogicalSize::new(640.0, 420.0));
                self.step = 11;
            }
            11 => {
                let size = browser
                    .window
                    .inner_size()
                    .to_logical::<f64>(browser.window.scale_factor());
                let rect = active.view.bounds().unwrap();
                let page_size = rect.size.to_logical::<f64>(browser.window.scale_factor());
                if (size.width - 640.0).abs() > 1.0
                    || (page_size.height - (size.height - BAR_HEIGHT - STATUS_HEIGHT)).abs() > 1.0
                {
                    return;
                }
                let proxy = browser.proxy.clone();
                browser.toolbar.evaluate_script_with_callback("({count:document.querySelectorAll('[role=tab]').length,selected:document.querySelector('[role=tab][aria-selected=true]')?.dataset.tabId,address:document.getElementById('address').value})",move|json|{let _=proxy.send_event(BrowserEvent::CheckResult(json));}).unwrap();
                self.step = 12;
            }
            12 => {
                if let Some(json) = self.received.take() {
                    let value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                    if value["count"] != 1
                        || value["address"] != ""
                        || value["selected"].as_str()
                            != Some(browser.tabs.active_id().unwrap().to_string().as_str())
                    {
                        self.failure = Some(format!("toolbar not synchronized: {json}"));
                        return;
                    }
                    self.step = 20;
                }
            }
            _ => {}
        }
    }
}
