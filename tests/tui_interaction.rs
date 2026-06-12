use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use rata::{
    RataProject,
    tui::{ResponseTab, TuiApp},
};
use tempfile::tempdir;

#[test]
fn tui_app_edits_url_from_key_input() {
    let mut app = TuiApp::new(None);

    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), None)
        .unwrap();
    for value in "http://localhost:8000/v1/models".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE), None)
            .unwrap();
    }
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), None)
        .unwrap();
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), None)
        .unwrap();

    assert_eq!(app.draft.url, "http://localhost:8000/v1/model");
}

#[test]
fn response_view_uses_tabs_and_pretty_json_body() {
    let mut app = TuiApp::new(None);
    app.response.body = r#"{"ok":true,"items":[{"id":1}]}"#.to_string();
    app.response.headers = vec![("content-type".to_string(), "application/json".to_string())];
    app.response.cookies = vec!["session=abc; Path=/".to_string()];

    assert_eq!(app.response_tabs(), ["Body".to_string(), "Headers (1)".to_string(), "Cookies (1)".to_string()]);
    assert_eq!(app.active_response_tab, ResponseTab::Body);
    assert_eq!(
        app.active_response_text(),
        "{\n  \"items\": [\n    {\n      \"id\": 1\n    }\n  ],\n  \"ok\": true\n}".into()
    );

    app.active_response_tab = ResponseTab::Headers;
    assert_eq!(app.active_response_text(), "content-type: application/json".into());

    app.active_response_tab = ResponseTab::Cookies;
    assert_eq!(app.active_response_text(), "session=abc; Path=/".into());
}

#[test]
fn response_tabs_are_clickable() {
    let mut app = TuiApp::new(None);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 9,
        row: 3,
        modifiers: KeyModifiers::NONE,
    }, None);
    assert_eq!(app.active_response_tab, ResponseTab::Headers);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 25,
        row: 3,
        modifiers: KeyModifiers::NONE,
    }, None);
    assert_eq!(app.active_response_tab, ResponseTab::Cookies);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 3,
        modifiers: KeyModifiers::NONE,
    }, None);
    assert_eq!(app.active_response_tab, ResponseTab::Body);
}

#[test]
fn tui_app_edits_url_and_sends_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(1);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "request was not sent");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept request: {error}"),
            }
        };
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.starts_with("GET /v1/models?limit=1 HTTP/1.1"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nx-request-id: req_123\r\nset-cookie: session=abc; Path=/; HttpOnly\r\ncontent-length: 12\r\n\r\n{\"ok\":true}\n",
            )
            .unwrap();
    });
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".rata")).unwrap();
    fs::write(
        tmp.path().join(".rata/openapi.yaml"),
        format!(
            r#"
openapi: 3.0.3
info:
  title: Example API
  version: 1.0.0
servers:
  - url: http://{address}
paths:
  /v1/models:
    get:
      tags: [models]
      summary: List models
      responses:
        "200":
          description: Models listed
"#
        ),
    )
    .unwrap();
    let project = RataProject::discover(tmp.path()).unwrap().unwrap();
    let mut app = TuiApp::new(Some(&project));

    app.edit_url(format!("http://{address}/v1/models?limit=1"));
    app.send().unwrap();
    server.join().unwrap();

    assert_eq!(app.draft.url, format!("http://{address}/v1/models?limit=1"));
    assert_eq!(app.response.status, Some(200));
    assert_eq!(app.response.body, "{\"ok\":true}\n");
    assert!(
        app.response
            .headers
            .iter()
            .any(|(name, value)| { name == "content-type" && value == "application/json" })
    );
    assert!(
        app.response
            .headers
            .iter()
            .any(|(name, value)| { name == "x-request-id" && value == "req_123" })
    );
    assert_eq!(
        app.response.cookies,
        vec!["session=abc; Path=/; HttpOnly".to_string()]
    );
    assert_eq!(app.response.error, None);
}
