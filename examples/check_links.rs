//! Local native WebView2 regression check. Pass --manual for a visible interactive window.
use plain_browser::{address, windows_links};
use std::{
    error::Error,
    time::{Duration, Instant},
};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
use windows::core::{HSTRING, w};
use wry::{PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewExtWindows};

enum CheckEvent {
    Open(windows_links::NewWindowRequest),
    Loaded(String),
}

fn click_in_test_page(page: &WebView, id: &str) {
    // Engine-level test gesture, restricted to our embedded fixture; no desktop input.
    let parameters = serde_json::json!({
        "expression": format!("document.getElementById({}).click()", serde_json::to_string(id).unwrap()),
        "userGesture": true,
    });
    unsafe {
        page.webview()
            .CallDevToolsProtocolMethod(
                w!("Runtime.evaluate"),
                &HSTRING::from(parameters.to_string()),
                &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(()))),
            )
            .expect("dispatch local test gesture");
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let automatic = !std::env::args().any(|arg| arg == "--manual");
    let mut event_loop = EventLoopBuilder::<CheckEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("PB Link Check")
        .with_visible(!automatic)
        .build(&event_loop)?;
    let profile =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/link-check-profile");
    let mut context = WebContext::new(Some(profile));
    let loaded = event_loop.create_proxy();
    let page = WebViewBuilder::new_with_web_context(&mut context)
        .with_visible(!automatic)
        .with_custom_protocol("link-check".into(), |_, request| {
            let path = request.uri().path();
            let auto = if path == "/start" { "<script>setTimeout(()=>window.open('/automatic','_blank'),100)</script>" } else { "" };
            let body = format!(r#"<!doctype html><html lang="ko"><meta charset="utf-8"><title>PB Link Check</title>
<style>body{{font:18px/2 'Segoe UI',sans-serif;padding:40px;background:#f5f8f5;color:#234235}}a,button{{display:block;margin:14px 0;padding:12px 20px;background:#fff;border:1px solid #8aa798;border-radius:8px;color:#165a3b;font:inherit;cursor:pointer;text-align:left}}h1{{font-size:24px}}</style>
<h1>링크 이동 확인: {path}</h1>
<p>이 창은 앱 내부 페이지로만 이동하는 확인용 창입니다.</p>
<a id="normal" href="/normal">일반 링크</a>
<a id="blank" href="/blank" target="_blank" rel="noopener">새 창 대상 링크</a>
<button id="script" onclick="window.open('/script','_blank')">클릭으로 여는 JavaScript 링크</button>
<a href="/start">확인 시작 화면으로</a>{auto}</html>"#);
            wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").body(body.into_bytes().into()).unwrap()
        })
        .with_url("http://link-check.localhost/start")
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) { let _ = loaded.send_event(CheckEvent::Loaded(url)); }
        })
        .build(&window)?;
    let links = event_loop.create_proxy();
    let _hook = windows_links::install(&page, move |request| {
        let _ = links.send_event(CheckEvent::Open(request));
    })?;
    let mut normal = false;
    let mut blank = false;
    let mut script = false;
    let mut auto_blocked = false;
    let start = Instant::now();
    event_loop.run_return(|event, _, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(1));
        if start.elapsed() > Duration::from_secs(if automatic { 20 } else { 180 }) {
            *flow = ControlFlow::Exit;
        }
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::UserEvent(CheckEvent::Open(request)) => {
                println!(
                    "new-window user={} target={}",
                    request.user_initiated, request.url
                );
                match address::clicked_link_target(&request.url, request.user_initiated) {
                    Ok(url) => {
                        page.load_url(&url).expect("navigate test link");
                    }
                    Err(message) => {
                        auto_blocked |=
                            request.url.ends_with("/automatic") && !request.user_initiated;
                        println!("denied: {message}");
                        if automatic && auto_blocked && !normal {
                            click_in_test_page(&page, "normal");
                        }
                    }
                }
            }
            Event::UserEvent(CheckEvent::Loaded(url)) => {
                normal |= url.ends_with("/normal");
                blank |= url.ends_with("/blank");
                script |= url.ends_with("/script");
                window.set_title(&format!(
                    "PB Link Check — {}",
                    url.rsplit('/').next().unwrap_or_default()
                ));
                println!("loaded {url}");
                if automatic {
                    if url.ends_with("/normal") {
                        click_in_test_page(&page, "blank");
                    } else if url.ends_with("/blank") {
                        click_in_test_page(&page, "script");
                    } else if url.ends_with("/script") {
                        *flow = ControlFlow::Exit;
                    }
                }
            }
            _ => {}
        }
    });
    println!(
        "normal={normal} target_blank={blank} onclick_window_open={script} automatic_popup_blocked={auto_blocked}"
    );
    if !(normal && blank && script && auto_blocked) {
        return Err("link checks incomplete".into());
    }
    println!("PASS: clicked links navigate in current view; automatic popup blocked.");
    Ok(())
}
