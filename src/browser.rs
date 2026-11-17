#[path = "interaction_check.rs"]
mod interaction_check;
#[path = "tab_check.rs"]
mod tab_check;

use plain_browser::{
    address,
    blocking::AdBlocker,
    tabs::{TabId, Tabs},
    windows_blocking, windows_links, windows_navigation,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, cell::RefCell, error::Error, path::PathBuf, rc::Rc};
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::{Icon, Window, WindowBuilder},
};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

const BAR_HEIGHT: f64 = 102.0;
const STATUS_HEIGHT: f64 = 30.0;
const HOME: &str = include_str!("ui/home.html");
const HOME_URL: &str = "http://plain-home.localhost/";
const TOOLBAR_URL: &str = "http://plain-ui.localhost/";
type AppResult<T> = Result<T, Box<dyn Error>>;

fn local_page(html: &'static str, path: &str) -> wry::http::Response<Cow<'static, [u8]>> {
    let (status, body) = if path == "/" {
        (200, html.as_bytes())
    } else {
        (404, b"Not found".as_slice())
    };
    wry::http::Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .body(Cow::Borrowed(body))
        .expect("valid static headers")
}
fn display_address(url: &str) -> String {
    if url == HOME_URL || url == "about:blank" {
        String::new()
    } else {
        url.into()
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Navigate { address: String },
    Back,
    Forward,
    Reload,
    Ready,
    ToggleBlocking,
    NewTab,
    SelectTab { id: TabId },
    CloseTab { id: TabId },
    MoveTab { id: TabId, before: Option<TabId> },
}
enum BrowserEvent {
    Command(Command),
    Page(TabId, PageEvent),
    CheckResult(String),
}
enum PageEvent {
    Navigation(String),
    Loaded(String),
    Title(String),
    Notice(String),
    OpenLink { url: String, user_initiated: bool },
    OpenExternal(String),
    Hover(String),
}
struct Tab {
    // Unregister callbacks before dropping this page's controller.
    _requests: windows_blocking::RequestHook,
    _links: windows_links::LinkHook,
    _navigation: windows_navigation::NavigationHook,
    view: WebView,
    intent: Rc<windows_navigation::NavigationIntent>,
    url: String,
    title: String,
    status: String,
    loading: bool,
    hovered_link: String,
}
#[derive(Serialize)]
struct TabSummary<'a> {
    id: TabId,
    title: &'a str,
    loading: bool,
}
#[derive(Serialize)]
struct ToolbarState<'a> {
    active_tab: TabId,
    tabs: Vec<TabSummary<'a>>,
    address: String,
    status: &'a str,
    back: bool,
    forward: bool,
    loading: bool,
    blocking_enabled: bool,
    hovered_link: &'a str,
}
fn bounds(window: &Window, toolbar: bool) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    Rect {
        position: LogicalPosition::new(0.0, if toolbar { 0.0 } else { BAR_HEIGHT }).into(),
        size: LogicalSize::new(
            size.width,
            if toolbar {
                size.height
            } else {
                (size.height - BAR_HEIGHT - STATUS_HEIGHT).max(1.0)
            },
        )
        .into(),
    }
}

fn create_tab(
    id: TabId,
    window: &Window,
    context: &mut WebContext,
    blocker: Rc<AdBlocker>,
    proxy: EventLoopProxy<BrowserEvent>,
    checking: bool,
) -> AppResult<Tab> {
    let top_url = Rc::new(RefCell::new(HOME_URL.to_owned()));
    let loaded = proxy.clone();
    let title = proxy.clone();
    let download = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(context)
        .with_custom_protocol("plain-home".into(), |_, request| {
            local_page(HOME, request.uri().path())
        })
        .with_url(HOME_URL)
        .with_bounds(bounds(window, false))
        .with_visible(false)
        .with_focused(false)
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = loaded.send_event(BrowserEvent::Page(id, PageEvent::Loaded(url)));
            }
        })
        .with_document_title_changed_handler(move |text| {
            let _ = title.send_event(BrowserEvent::Page(id, PageEvent::Title(text)));
        })
        .with_download_started_handler(move |_, _| {
            let _ = download.send_event(BrowserEvent::Page(
                id,
                PageEvent::Notice("파일 다운로드는 다음 단계에서 지원합니다.".into()),
            ));
            false
        });
    let builder = if checking {
        builder.with_custom_protocol("plain-check".into(),|_,request|{
        let label=request.uri().path().trim_start_matches('/');
        let body=format!("<!doctype html><meta charset='utf-8'><title>Tab {label}</title><h1>Tab {label}</h1><input id='note' value='{label}'><p><a id='same' href='/same'>Same site</a></p><p><a id='external' href='http://plain-check.other.test/external'>Other site</a></p><p><a id='blank' href='http://plain-check.other.test/blank' target='_blank'>Other site blank</a></p>");
        wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").body(body.into_bytes().into()).unwrap()
    })
    } else {
        builder
    };
    let view = builder.build_as_child(window)?;
    let intent = Rc::new(windows_navigation::NavigationIntent::default());
    let navigation = proxy.clone();
    let navigation_hook = windows_navigation::install(
        &view,
        Rc::clone(&top_url),
        Rc::clone(&intent),
        move |event| {
            use windows_navigation::NavigationEvent;
            let event = match event {
                NavigationEvent::Starting(url) => PageEvent::Navigation(url),
                NavigationEvent::NewTab(url) => PageEvent::OpenExternal(url),
                NavigationEvent::Hover(url) => PageEvent::Hover(url),
                NavigationEvent::Notice(message) => PageEvent::Notice(message),
            };
            let _ = navigation.send_event(BrowserEvent::Page(id, event));
        },
    )?;
    let links = proxy.clone();
    let link_hook = windows_links::install(&view, move |request| {
        let _ = links.send_event(BrowserEvent::Page(
            id,
            PageEvent::OpenLink {
                url: request.url,
                user_initiated: request.user_initiated,
            },
        ));
    })?;
    let requests = windows_blocking::install(&view, blocker, top_url, move |result| {
        if let Err(message) = result {
            let _ = proxy.send_event(BrowserEvent::Page(id, PageEvent::Notice(message)));
        }
    })?;
    Ok(Tab {
        _requests: requests,
        _links: link_hook,
        _navigation: navigation_hook,
        view,
        intent,
        url: HOME_URL.into(),
        title: "새 탭".into(),
        status: "준비".into(),
        loading: true,
        hovered_link: String::new(),
    })
}

struct Browser {
    tabs: Tabs<Tab>,
    toolbar: WebView,
    context: WebContext,
    window: Window,
    blocker: Rc<AdBlocker>,
    proxy: EventLoopProxy<BrowserEvent>,
    checking: bool,
}
impl Browser {
    fn new(event_loop: &EventLoop<BrowserEvent>, checking: bool) -> AppResult<Self> {
        let window = WindowBuilder::new()
            .with_title("Plain Browser")
            .with_visible(!checking)
            .with_window_icon(Some(Icon::from_rgba(
                include_bytes!("../assets/plain-browser.rgba").to_vec(),
                64,
                64,
            )?))
            .with_inner_size(LogicalSize::new(1120.0, 780.0))
            .with_min_inner_size(LogicalSize::new(640.0, 420.0))
            .build(event_loop)?;
        let path = if checking {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/tabs-check-profile")
        } else {
            std::env::current_exe()?
                .parent()
                .ok_or("실행 파일 폴더를 찾을 수 없습니다.")?
                .join("data/webview2")
        };
        std::fs::create_dir_all(&path)?;
        let mut context = WebContext::new(Some(path));
        let proxy = event_loop.create_proxy();
        let ipc = proxy.clone();
        let toolbar = WebViewBuilder::new_with_web_context(&mut context)
            .with_custom_protocol("plain-ui".into(), |_, request| {
                local_page(include_str!("ui/toolbar.html"), request.uri().path())
            })
            .with_url(TOOLBAR_URL)
            .with_bounds(bounds(&window, true))
            .with_visible(!checking)
            .with_navigation_handler(|url| url == TOOLBAR_URL)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_ipc_handler(move |request| {
                if request.uri().host() == Some("plain-ui.localhost")
                    && request.body().len() <= 8192
                    && let Ok(command) = serde_json::from_str::<Command>(request.body())
                {
                    let _ = ipc.send_event(BrowserEvent::Command(command));
                }
            })
            .build_as_child(&window)?;
        let mut browser = Self {
            tabs: Tabs::default(),
            toolbar,
            context,
            window,
            blocker: Rc::new(AdBlocker::default()),
            proxy,
            checking,
        };
        browser.add_tab()?;
        Ok(browser)
    }
    fn add_tab(&mut self) -> AppResult<TabId> {
        if let Some(tab) = self.tabs.active_id().and_then(|id| self.tabs.get_mut(id)) {
            tab.hovered_link.clear();
        }
        let id = self.tabs.create(|id| {
            create_tab(
                id,
                &self.window,
                &mut self.context,
                Rc::clone(&self.blocker),
                self.proxy.clone(),
                self.checking,
            )
        })?;
        self.show_active()?;
        Ok(id)
    }
    fn show_active(&self) -> wry::Result<()> {
        for (id, tab) in self.tabs.iter() {
            tab.view
                .set_visible(!self.checking && Some(id) == self.tabs.active_id())?;
        }
        Ok(())
    }
    fn resize(&self) -> wry::Result<()> {
        self.toolbar.set_bounds(bounds(&self.window, true))?;
        for (_, tab) in self.tabs.iter() {
            tab.view.set_bounds(bounds(&self.window, false))?;
        }
        Ok(())
    }
    fn refresh(&self) {
        let Some(active) = self.tabs.active() else {
            return;
        };
        let state = ToolbarState {
            active_tab: self.tabs.active_id().unwrap(),
            tabs: self
                .tabs
                .iter()
                .map(|(id, tab)| TabSummary {
                    id,
                    title: &tab.title,
                    loading: tab.loading,
                })
                .collect(),
            address: display_address(&active.url),
            status: &active.status,
            back: active.view.can_go_back().unwrap_or(false),
            forward: active.view.can_go_forward().unwrap_or(false),
            loading: active.loading,
            blocking_enabled: self.blocker.enabled(),
            hovered_link: &active.hovered_link,
        };
        self.window.set_title(&format!(
            "{} — Plain Browser",
            active.title.chars().take(100).collect::<String>()
        ));
        if let Ok(json) = serde_json::to_string(&state)
            && let Err(error) = self
                .toolbar
                .evaluate_script(&format!("window.updateState({json})"))
        {
            eprintln!("주소창 갱신 실패: {error}");
        }
    }
    fn notice(&mut self, message: String) {
        if let Some(tab) = self.tabs.active_id().and_then(|id| self.tabs.get_mut(id)) {
            tab.status = message;
            tab.loading = false;
        }
    }
    fn command(&mut self, command: Command) -> AppResult<()> {
        match command {
            Command::NewTab => {
                self.add_tab()?;
                if !self.checking {
                    self.toolbar.focus()?;
                    self.toolbar.evaluate_script("window.focusAddress()")?;
                }
            }
            Command::SelectTab { id } => {
                if self.tabs.select(id) {
                    self.tabs.get_mut(id).unwrap().hovered_link.clear();
                    self.show_active()?;
                    if !self.checking {
                        self.tabs.active().unwrap().view.focus()?;
                    }
                }
            }
            Command::CloseTab { id } => {
                if self.tabs.get(id).is_none() {
                    return Ok(());
                }
                if self.tabs.len() == 1 {
                    self.add_tab()?;
                }
                self.tabs.get(id).unwrap().view.set_visible(false)?;
                drop(self.tabs.close(id));
                self.show_active()?;
            }
            Command::Ready => {}
            Command::MoveTab { id, before } => {
                self.tabs.move_before(id, before);
            }
            command => {
                let Some(tab) = self.tabs.active_id().and_then(|id| self.tabs.get_mut(id)) else {
                    return Ok(());
                };
                match command {
                    Command::Navigate { address: input } => match address::normalize(&input) {
                        Ok(url) => {
                            tab.loading = true;
                            tab.url = url.clone();
                            tab.intent.navigate(&tab.view, &url)?;
                        }
                        Err(message) => tab.status = message.into(),
                    },
                    Command::Back => {
                        tab.loading = true;
                        tab.view.go_back()?;
                    }
                    Command::Forward => {
                        tab.loading = true;
                        tab.view.go_forward()?;
                    }
                    Command::Reload => {
                        tab.loading = true;
                        tab.view.reload()?;
                    }
                    Command::ToggleBlocking => {
                        self.blocker.set_enabled(!self.blocker.enabled());
                        tab.loading = true;
                        tab.view.reload()?;
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }
    fn page_event(&mut self, id: TabId, event: PageEvent) {
        // Cross-site navigation is deferred until the native callback has returned.
        let external = match &event {
            PageEvent::OpenExternal(url) => Some(url.clone()),
            PageEvent::OpenLink {
                url,
                user_initiated: true,
            } => self
                .tabs
                .get(id)
                .filter(|tab| address::different_site(&tab.url, url))
                .and_then(|_| address::clicked_link_target(url, true).ok()),
            _ => None,
        };
        if let Some(target) = external {
            if self.tabs.get(id).is_none() {
                return;
            }
            if let Err(error) = self.open_external(&target)
                && let Some(tab) = self.tabs.get_mut(id)
            {
                tab.status = format!("새 탭을 열지 못했습니다: {error}");
            }
            return;
        }
        let Some(tab) = self.tabs.get_mut(id) else {
            return;
        };
        match event {
            PageEvent::Navigation(url) => {
                tab.hovered_link.clear();
                tab.url = url;
                tab.loading = true;
                tab.status = "페이지를 여는 중…".into();
            }
            PageEvent::Loaded(url) => {
                tab.hovered_link.clear();
                tab.url = url;
                tab.loading = false;
                tab.status = "완료".into();
            }
            PageEvent::Title(title) => {
                tab.title = if title.trim().is_empty() {
                    "새 탭".into()
                } else {
                    title.chars().take(150).collect()
                }
            }
            PageEvent::Notice(message) => tab.status = message,
            PageEvent::OpenLink {
                url,
                user_initiated,
            } => match address::clicked_link_target(&url, user_initiated) {
                Ok(target) => {
                    if let Err(error) = tab.intent.navigate(&tab.view, &target) {
                        tab.status = format!("링크를 열지 못했습니다: {error}");
                    }
                }
                Err(message) => tab.status = message.into(),
            },
            PageEvent::Hover(url) => tab.hovered_link = url,
            PageEvent::OpenExternal(_) => {}
        }
    }
    fn open_external(&mut self, url: &str) -> AppResult<()> {
        let id = self.add_tab()?;
        let tab = self.tabs.get_mut(id).unwrap();
        tab.loading = true;
        tab.url = url.to_owned();
        tab.intent.navigate(&tab.view, url)?;
        if !self.checking {
            tab.view.focus()?;
        }
        Ok(())
    }
}

pub fn run() -> AppResult<()> {
    let interaction_checking =
        cfg!(debug_assertions) && std::env::args().any(|arg| arg == "--check-interactions");
    let checking = interaction_checking
        || (cfg!(debug_assertions) && std::env::args().any(|arg| arg == "--check-tabs"));
    let mut event_loop = EventLoopBuilder::<BrowserEvent>::with_user_event().build();
    let mut browser = Browser::new(&event_loop, checking)?;
    let mut check = (checking && !interaction_checking).then(tab_check::TabCheck::new);
    let mut interactions = interaction_checking.then(interaction_check::InteractionCheck::new);
    let mut ready = false;
    event_loop.run_return(|event, _, flow| {
        let mut refresh = false;
        *flow = if checking {
            ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(100),
            )
        } else {
            ControlFlow::Wait
        };
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                if let Err(error) = browser.resize() {
                    eprintln!("창 크기 변경 실패: {error}");
                }
            }
            Event::UserEvent(BrowserEvent::Command(command)) => {
                refresh = true;
                ready |= matches!(command, Command::Ready);
                if let Err(error) = browser.command(command) {
                    browser.notice(format!("요청을 처리하지 못했습니다: {error}"));
                }
            }
            Event::UserEvent(BrowserEvent::Page(id, event)) => {
                refresh = true;
                browser.page_event(id, event);
            }
            Event::UserEvent(BrowserEvent::CheckResult(json)) => {
                if let Some(interactions) = &mut interactions {
                    interactions.receive(json);
                } else if let Some(check) = &mut check {
                    check.receive(json);
                }
            }
            _ => {}
        }
        if ready && refresh {
            browser.refresh();
        }
        if let Some(check) = &mut check {
            check.advance(ready, &browser);
            if check.finished() {
                *flow = ControlFlow::Exit;
            }
        }
        if let Some(interactions) = &mut interactions {
            interactions.advance(ready, &browser);
            if interactions.finished() {
                *flow = ControlFlow::Exit;
            }
        }
    });
    if let Some(check) = check {
        check.result()?;
    }
    if let Some(interactions) = interactions {
        interactions.result()?;
    }
    Ok(())
}
