//! Local-only end-to-end check: a real hidden WebView2, no remote test services.
use plain_browser::{blocking::AdBlocker, windows_blocking};
use serde::Deserialize;
use std::{
    cell::RefCell,
    error::Error,
    io::{Read, Write},
    net::TcpListener,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{WebContext, WebViewBuilder};

#[derive(Debug, Deserialize)]
struct Report {
    static_ad: bool,
    late_ad: bool,
    image_ad: bool,
    fetch_ad: bool,
    publisher: bool,
    normal: bool,
    inline: bool,
}

fn fixture(port: u16) -> String {
    include_str!("fixtures/adblock.html").replace("__PORT__", &port.to_string())
}

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let running = Arc::new(AtomicBool::new(true));
    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_running = Arc::clone(&running);
    let server_hits = Arc::clone(&hits);
    let server = thread::spawn(move || {
        let mut clients = Vec::new();
        while server_running.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(10));
                continue;
            };
            let connection_hits = Arc::clone(&server_hits);
            clients.push(thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buffer = Vec::new();
            let mut chunk = [0; 2048];
            while !buffer.windows(4).any(|part| part == b"\r\n\r\n") && buffer.len() < 16384 {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => buffer.extend_from_slice(&chunk[..size]),
                }
            }
            if buffer.is_empty() {
                return;
            }
            let request = String::from_utf8_lossy(&buffer);
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
            connection_hits.lock().unwrap().push(path.clone());
            let (mime, body) = match path.as_str() {
                "/" => ("text/html; charset=utf-8", fixture(port)),
                "/ads/publisher.js" => ("text/javascript", "window.publisherLoaded=true;".into()),
                "/content.js" => ("text/javascript", "window.normalLoaded=true;".into()),
                "/ad-network/banner.svg" => (
                    "image/svg+xml",
                    "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>".into(),
                ),
                "/ad-network/frame.html" => ("text/html", "publisher-independent ad frame".into()),
                _ => ("text/javascript", "/* ad payload reached server */".into()),
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            }));
        }
        for client in clients {
            let _ = client.join();
        }
    });

    let mut event_loop = EventLoopBuilder::<Report>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Plain Browser request check")
        .with_visible(false)
        .build(&event_loop)?;
    let profile =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/adblock-check-profile");
    let mut context = WebContext::new(Some(profile));
    let proxy = event_loop.create_proxy();
    let page = WebViewBuilder::new_with_web_context(&mut context)
        .with_visible(false)
        .with_ipc_handler(move |request| {
            if let Ok(report) = serde_json::from_str::<Report>(request.body()) {
                let _ = proxy.send_event(report);
            }
        })
        .build(&window)?;
    let source = format!("http://localhost:{port}/");
    let blocker = Rc::new(AdBlocker::from_rules("/ad-network/\n/ads/publisher.js"));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let callback_errors = Rc::clone(&errors);
    let _hook = windows_blocking::install(
        &page,
        Rc::clone(&blocker),
        Rc::new(RefCell::new(source.clone())),
        move |result| {
            if let Err(error) = result {
                callback_errors.borrow_mut().push(error);
            }
        },
    )?;
    page.load_url(&source)?;
    let start = Instant::now();
    let mut phase = 0;
    let mut failure = None;
    let mut hits_when_disabled = 0;
    event_loop.run_return(|event, _, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
        if start.elapsed() > Duration::from_secs(30) {
            failure = Some("local WebView2 check timed out".to_owned());
            *flow = ControlFlow::Exit;
            return;
        }
        if let Event::UserEvent(report) = event {
            let expected_ad = phase == 1;
            let preserved = report.publisher && report.normal && report.inline;
            let filtered = [
                report.static_ad,
                report.late_ad,
                report.image_ad,
                report.fetch_ad,
            ]
            .iter()
            .all(|&loaded| loaded == expected_ad);
            let ad_hits = hits
                .lock()
                .unwrap()
                .iter()
                .filter(|p| p.starts_with("/ad-network/"))
                .count();
            let server_behavior = match phase {
                0 => ad_hits == 0 && blocker.blocked() == 5,
                1 => { hits_when_disabled = ad_hits; ad_hits == 5 && blocker.blocked() == 5 },
                _ => ad_hits == hits_when_disabled && blocker.blocked() == 10,
            };
            println!(
                "phase={phase} enabled={} blocked={} ad_server_hits={ad_hits} report={report:?}",
                blocker.enabled(),
                blocker.blocked()
            );
            if !preserved
                || !filtered
                || !errors.borrow().is_empty()
                || !server_behavior
            {
                failure = Some(format!(
                    "unexpected filtering or publisher preservation: {report:?}; errors={:?}; hits={:?}",
                    errors.borrow(), hits.lock().unwrap()
                ));
                *flow = ControlFlow::Exit;
            } else if phase < 2 {
                phase += 1;
                blocker.set_enabled(phase != 1);
                if let Err(error) = page.reload() {
                    failure = Some(error.to_string());
                    *flow = ControlFlow::Exit;
                }
            } else {
                *flow = ControlFlow::Exit;
            }
        }
    });
    running.store(false, Ordering::Relaxed);
    server.join().expect("local server stopped");
    if let Some(error) = failure {
        return Err(error.into());
    }
    if phase != 2 {
        return Err("not all check phases completed".into());
    }
    println!(
        "PASS: native requests blocked before reaching server; inline/publisher/normal content preserved; off/on verified."
    );
    Ok(())
}
