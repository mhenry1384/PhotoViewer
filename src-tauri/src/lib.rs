use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use tauri::State;

struct AppState {
    initial_file: Mutex<Option<String>>,
}

static TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();

// Single background prefetch worker — all conversion requests are serialized through
// one thread to avoid concurrent WIC usage crashing the process.
static PREFETCH_TX: OnceLock<Mutex<mpsc::Sender<String>>> = OnceLock::new();

fn init_prefetch_worker() {
    PREFETCH_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for file_path in rx {
                let ext = Path::new(&file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if matches!(ext.as_str(), "heic" | "heif") {
                    if let Ok(temp) = temp_path_for(&file_path) {
                        if !Path::new(&temp).exists() {
                            let _ = convert_heic_to_jpeg(&file_path, &temp);
                        }
                    }
                }
            }
        });
        Mutex::new(tx)
    });
}

fn get_temp_dir() -> &'static PathBuf {
    TEMP_DIR.get_or_init(|| {
        let mut dir = std::env::temp_dir();
        dir.push(format!("photo_viewer_{}", std::process::id()));
        dir
    })
}

fn ensure_temp_dir() -> Result<&'static PathBuf, String> {
    let dir = get_temp_dir();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn temp_path_for(original: &str) -> Result<String, String> {
    let dir = ensure_temp_dir()?;
    let mut hasher = DefaultHasher::new();
    original.hash(&mut hasher);
    let hash = hasher.finish();
    Ok(dir
        .join(format!("{:016x}.jpg", hash))
        .to_string_lossy()
        .into_owned())
}

// Max pixel dimension for converted HEIC — keeps decoding fast on large sensor photos.
const HEIC_MAX_DIM: u32 = 2048;

fn convert_heic_to_jpeg(heic_path: &str, jpeg_path: &str) -> Result<(), String> {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::GENERIC_READ,
            Graphics::Imaging::{
                CLSID_WICImagingFactory, IWICBitmapScaler, IWICFormatConverter,
                IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
                WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand, WICRect,
                GUID_WICPixelFormat24bppBGR,
            },
            System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
        },
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("WIC init failed: {e}"))?;

        let path_wide: Vec<u16> = heic_path.encode_utf16().chain(Some(0)).collect();
        let decoder = factory
            .CreateDecoderFromFilename(
                PCWSTR::from_raw(path_wide.as_ptr()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )
            .map_err(|e| format!("HEIC decode failed (HEIF Image Extensions required): {e}"))?;

        let frame = decoder
            .GetFrame(0)
            .map_err(|e| format!("Frame read failed: {e}"))?;

        // Scale down large photos to HEIC_MAX_DIM before pixel copy — dramatically
        // reduces memory and encode time for high-resolution sensor images.
        let mut orig_w = 0u32;
        let mut orig_h = 0u32;
        frame.GetSize(&mut orig_w, &mut orig_h)
            .map_err(|e| format!("GetSize failed: {e}"))?;

        let scale = (HEIC_MAX_DIM as f64 / orig_w.max(orig_h) as f64).min(1.0);
        let out_w = ((orig_w as f64 * scale).round() as u32).max(1);
        let out_h = ((orig_h as f64 * scale).round() as u32).max(1);

        let scaler: IWICBitmapScaler = factory
            .CreateBitmapScaler()
            .map_err(|e| format!("Scaler create failed: {e}"))?;
        scaler
            .Initialize(&frame, out_w, out_h, WICBitmapInterpolationModeFant)
            .map_err(|e| format!("Scaler init failed: {e}"))?;

        let converter: IWICFormatConverter = factory
            .CreateFormatConverter()
            .map_err(|e| format!("Format converter failed: {e}"))?;
        converter
            .Initialize(
                &scaler,
                &GUID_WICPixelFormat24bppBGR,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .map_err(|e| format!("Converter init failed: {e}"))?;

        let stride = out_w * 3;
        let mut buffer = vec![0u8; (stride * out_h) as usize];

        let rect = WICRect {
            X: 0,
            Y: 0,
            Width: out_w as i32,
            Height: out_h as i32,
        };
        converter
            .CopyPixels(&rect, stride, &mut buffer)
            .map_err(|e| format!("CopyPixels failed: {e}"))?;

        // WIC returns BGR; image crate expects RGB
        for chunk in buffer.chunks_exact_mut(3) {
            chunk.swap(0, 2);
        }

        let img = image::RgbImage::from_raw(out_w, out_h, buffer)
            .ok_or("Failed to construct image from pixel data")?;
        img.save_with_format(jpeg_path, image::ImageFormat::Jpeg)
            .map_err(|e| format!("JPEG save failed: {e}"))?;

        Ok(())
    }
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
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "heic" | "heif") {
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
fn get_display_path(file_path: String) -> Result<String, String> {
    let ext = Path::new(&file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if matches!(ext.as_str(), "heic" | "heif") {
        let temp = temp_path_for(&file_path)?;
        if !Path::new(&temp).exists() {
            convert_heic_to_jpeg(&file_path, &temp)?;
        }
        Ok(temp)
    } else {
        Ok(file_path)
    }
}

#[tauri::command]
fn prefetch_display_paths(file_paths: Vec<String>) {
    init_prefetch_worker();
    if let Some(tx) = PREFETCH_TX.get() {
        if let Ok(sender) = tx.lock() {
            for path in file_paths {
                let _ = sender.send(path);
            }
        }
    }
}

#[tauri::command]
fn cleanup_temp_files() {
    if let Some(dir) = TEMP_DIR.get() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                std::fs::remove_file(entry.path()).ok();
            }
        }
    }
}

#[tauri::command]
fn trash_file(path: String) -> Result<(), String> {
    trash::delete(&path).map_err(|e| e.to_string())
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    // args[0] is the executable; args[1] (if present) is the file to open
    let initial_file = args.get(1).cloned();

    init_prefetch_worker();

    tauri::Builder::default()
        .manage(AppState {
            initial_file: Mutex::new(initial_file),
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_file,
            get_images_in_folder,
            get_display_path,
            prefetch_display_paths,
            cleanup_temp_files,
            trash_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
