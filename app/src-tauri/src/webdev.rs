//! For development only (the `web-dev` feature, never in a release): the
//! app's commands for the web UI in an ordinary browser, for prototyping
//! the UI with hot reload and the browser's phone and tablet views.
//!
//! The desktop app runs with its window hidden (it only runs the page's
//! commands; stop it with Ctrl+C where `npm run web` runs); this serves
//! its commands on
//! 127.0.0.1:1422 (`POST /__ipc/<command>`, the arguments as JSON), each
//! run by the app's own handlers as if the window had called it. Vite
//! passes `/__ipc` on, and the page in a browser sends its calls there
//! (`src/lib/webdev.ts`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager};

pub const ADDRESS: &str = "127.0.0.1:1422";

/// Serve the commands, on a thread of their own, the window hidden.
pub fn serve(handle: AppHandle) {
    std::thread::spawn(move || {
        // The window is made as the app starts: hide it once it is there.
        for _ in 0..100 {
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.hide();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let listener = match TcpListener::bind(ADDRESS) {
            Ok(l) => l,
            Err(e) => return eprintln!("web-dev: cannot listen on {ADDRESS}: {e}"),
        };
        eprintln!("web-dev: open http://localhost:1420 in a browser (Ctrl+C here stops it)");
        for stream in listener.incoming().flatten() {
            let handle = handle.clone();
            std::thread::spawn(move || {
                if let Err(e) = answer(&handle, stream) {
                    eprintln!("web-dev: {e}");
                }
            });
        }
    });
}

/// One request: `POST /__ipc/<command>` with the arguments as JSON.
fn answer(handle: &AppHandle, mut stream: TcpStream) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(|e| e.to_string())?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;

    let Some(cmd) = path.strip_prefix("/__ipc/") else {
        return reply(&mut stream, 404, "\"not a command\"");
    };
    let cmd = cmd.replace("%7C", "|");
    let args: serde_json::Value =
        if body.is_empty() { serde_json::json!({}) } else { serde_json::from_slice(&body).map_err(|e| e.to_string())? };
    let (status, text) = run(handle, &cmd, args);
    reply(&mut stream, status, &text)
}

/// Run a command as the main window would, and wait for its answer.
fn run(handle: &AppHandle, cmd: &str, args: serde_json::Value) -> (u16, String) {
    let Some(window) = handle.get_webview_window("main") else {
        return (500, "\"the app's window is not open\"".into());
    };
    let Ok(url) = window.url() else {
        return (500, "\"the app's window has no address\"".into());
    };
    let request = InvokeRequest {
        cmd: cmd.to_string(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url,
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: handle.invoke_key().to_string(),
    };
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    window.as_ref().clone().on_message(
        request,
        Box::new(move |_webview, _cmd, response, _callback, _error| {
            let _ = tx.send(response);
        }),
    );
    match rx.recv() {
        Ok(InvokeResponse::Ok(InvokeResponseBody::Json(json))) => (200, json),
        Ok(InvokeResponse::Ok(InvokeResponseBody::Raw(bytes))) => (200, serde_json::to_string(&bytes).unwrap_or_default()),
        Ok(InvokeResponse::Err(e)) => (400, e.0.to_string()),
        Err(_) => (500, "\"no answer\"".into()),
    }
}

fn reply(stream: &mut TcpStream, status: u16, json: &str) -> Result<(), String> {
    let head =
        format!("HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", json.len());
    stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(json.as_bytes())).map_err(|e| e.to_string())
}
