// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop shell. All framing runs in the web view through the same
//! WebAssembly core the browser build uses, so this stays a thin host. The
//! one exception is the local model for plain-English edits, which is too
//! big for the web view and runs here.

use sticks_intent::Answers;
use sticks_local::Query;
use tauri_plugin_dialog::DialogExt;

/// Answers a request's questions with the model on this machine, the way
/// Jev would: a pick from each question's options and how sure it was.
#[tauri::command]
async fn ask_local(
    app: tauri::AppHandle,
    request: String,
    design: String,
    asks: Vec<Query>,
) -> Result<Answers, String> {
    tauri::async_runtime::spawn_blocking(move || local::ask(&app, &request, &design, &asks))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(feature = "local-model")]
mod local {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use sticks_intent::Answers;
    use sticks_local::{find_model, Answerer, LocalModel, Query};
    use tauri::Manager;

    /// Loaded on the first request and kept.
    static MODEL: Mutex<Option<LocalModel>> = Mutex::new(None);

    pub fn ask(
        app: &tauri::AppHandle,
        request: &str,
        design: &str,
        asks: &[Query],
    ) -> Result<Answers, String> {
        let mut model = MODEL.lock().map_err(|e| e.to_string())?;
        if model.is_none() {
            let path = find_model(&dirs(app).iter().map(PathBuf::as_path).collect::<Vec<_>>())
                .map_err(|e| e.to_string())?;
            *model = Some(LocalModel::load(&path).map_err(|e| e.to_string())?);
        }
        let model = model.as_mut().expect("loaded above");
        model
            .answer(request, design, asks)
            .map_err(|e| e.to_string())
    }

    /// Where to look for the model: the app's data folder, then, in a dev
    /// build, the repository's models/ folder that `npm run model:get` fills.
    fn dirs(app: &tauri::AppHandle) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = app
            .path()
            .app_data_dir()
            .map(|d| d.join("models"))
            .into_iter()
            .collect();
        if cfg!(debug_assertions) {
            dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"));
        }
        dirs
    }
}

#[cfg(not(feature = "local-model"))]
mod local {
    use sticks_intent::Answers;
    use sticks_local::Query;

    pub fn ask(_: &tauri::AppHandle, _: &str, _: &str, _: &[Query]) -> Result<Answers, String> {
        Err("This build has no local model. Run `npm run tauri dev -- --features local-model` to add one.".into())
    }
}

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
        .invoke_handler(tauri::generate_handler![ask_local, save_file])
        .run(tauri::generate_context!())
        .expect("error while running Sticks");
}
