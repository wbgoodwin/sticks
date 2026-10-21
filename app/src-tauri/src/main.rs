// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop shell. All framing runs in the web view through the same
//! WebAssembly core the browser build uses, so this stays a thin host.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// Asks where to save `contents`, suggesting `name`, and writes it there.
/// False when the person cancels. The web view can't save files itself: a
/// download link does nothing in the desktop window.
#[tauri::command]
async fn save_file(
    app: tauri::AppHandle,
    name: String,
    contents: String,
    kind: String,
    extension: String,
) -> Result<bool, String> {
    let picked = app
        .dialog()
        .file()
        .set_file_name(&name)
        .add_filter(kind, &[extension.as_str()])
        .blocking_save_file();
    let Some(path) = picked else { return Ok(false) };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// Opens an https page in the person's browser: Stripe's checkout and
/// billing pages.
#[tauri::command]
fn open_in_browser(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Sticks only opens https pages.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Signs in to a Sticks account: opens the sign-in page `url` in the
/// browser, then waits on `localhost:port` for the browser to come back to
/// /auth/callback, and returns that address's query (the code and state).
/// The page itself finishes the sign-in.
#[tauri::command]
async fn sign_in_in_browser(app: tauri::AppHandle, url: String, port: u16) -> Result<String, String> {
    // Listen before opening the browser, so the answer can't arrive first.
    // "localhost" can mean either address, so both are tried.
    let listeners: Vec<TcpListener> = [SocketAddr::from(([127, 0, 0, 1], port)), SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 1], port))]
        .into_iter()
        .filter_map(|addr| TcpListener::bind(addr).ok())
        .collect();
    if listeners.is_empty() {
        return Err(format!("Couldn't listen on port {port} for the sign-in. Is another Sticks window signing in?"));
    }
    for l in &listeners {
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || wait_for_callback(&listeners, Duration::from_secs(600)))
        .await
        .map_err(|e| e.to_string())?
}

fn wait_for_callback(listeners: &[TcpListener], timeout: Duration) -> Result<String, String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        for l in listeners {
            if let Ok((stream, _)) = l.accept() {
                if let Some(query) = answer(stream) {
                    return Ok(query);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("The sign-in took too long. Try again.".into())
}

/// Reads one request. For /auth/callback, thanks the browser and returns
/// the query; anything else (a favicon) gets a 404.
fn answer(mut stream: TcpStream) -> Option<String> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?;
    let query = target.strip_prefix("/auth/callback").and_then(|rest| rest.strip_prefix('?').or(Some(rest)));
    let (status, body) = match query {
        Some(_) => ("200 OK", SIGNED_IN_PAGE),
        None => ("404 Not Found", ""),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    query.map(str::to_owned)
}

const SIGNED_IN_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Sticks</title>
<style>body{font:16px/1.5 -apple-system,system-ui,sans-serif;display:grid;place-items:center;min-height:90vh;margin:0;color:#24221e;background:#f6f4ef}
@media(prefers-color-scheme:dark){body{color:#ecebe6;background:#161714}}main{text-align:center;padding:16px}</style>
<main><h1>Signed in</h1><p>You can close this tab and go back to Sticks.</p></main>"#;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_file, open_in_browser, sign_in_in_browser])
        .run(tauri::generate_context!())
        .expect("error while running Sticks");
}
