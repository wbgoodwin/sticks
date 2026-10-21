// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop shell. All framing runs in the web view through the same
//! WebAssembly core the browser build uses, so this stays a thin host.

use tauri_plugin_dialog::DialogExt;

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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_file])
        .run(tauri::generate_context!())
        .expect("error while running Sticks");
}
