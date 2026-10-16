// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop shell. All framing runs in the web view through the same
//! WebAssembly core the browser build uses, so this stays a thin host.

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running Sticks");
}
