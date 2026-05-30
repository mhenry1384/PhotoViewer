use std::path::Path;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    initial_file: Mutex<Option<String>>,
}

#[tauri::command]
fn get_initial_file(state: State<AppState>) -> Option<String> {
    state.initial_file.lock().unwrap().clone()
}

#[tauri::command]
fn get_images_in_folder(folder: String) -> Vec<String> {
    let path = Path::new(&folder);
    let mut images: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if let Some(ext) = p.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png") {
                    if let Some(s) = p.to_str() {
                        images.push(s.to_string());
                    }
                }
            }
        }
    }

    images.sort_by(|a, b| {
        a.to_lowercase().cmp(&b.to_lowercase())
    });

    images
}

#[tauri::command]
fn trash_file(path: String) -> Result<(), String> {
    trash::delete(&path).map_err(|e| e.to_string())
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    // args[0] is the executable; args[1] (if present) is the file to open
    let initial_file = args.get(1).cloned();

    tauri::Builder::default()
        .manage(AppState {
            initial_file: Mutex::new(initial_file),
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_file,
            get_images_in_folder,
            trash_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
