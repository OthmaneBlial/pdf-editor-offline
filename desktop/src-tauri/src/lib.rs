use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, State};

const SIDECAR_NAME: &str = "pdf-editor-offline-api";

struct DesktopState {
    api_base_url: String,
    api_token: String,
    recent_files_path: PathBuf,
    sidecar: Mutex<Option<Child>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopApiConnection {
    base_url: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct DesktopFilePayload {
    name: String,
    size: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentFile {
    name: String,
    size: u64,
    #[serde(default)]
    path: Option<String>,
    #[serde(rename = "lastOpened")]
    last_opened: String,
}

fn read_recent_files(path: &Path) -> Vec<RecentFile> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

fn write_recent_files(path: &Path, files: &[RecentFile]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let content = serde_json::to_string_pretty(files).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn wait_for_backend(port: u16) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    Err("Timed out waiting for the PDF Editor Offline API sidecar".to_string())
}

fn stop_sidecar(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn start_sidecar(app: &tauri::App, port: u16, api_token: &str) -> Result<Child, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve application data directory: {error}"))?;
    let app_cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Could not resolve application cache directory: {error}"))?;
    let storage_dir = app_data_dir.join("storage");
    let temp_dir = app_cache_dir.join("temp");

    fs::create_dir_all(&storage_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;

    let executable_name = if cfg!(target_os = "windows") {
        format!("{SIDECAR_NAME}.exe")
    } else {
        SIDECAR_NAME.to_string()
    };
    let executable = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Could not resolve bundled resource directory: {error}"))?
        .join("resources")
        .join("sidecar")
        .join(executable_name);
    if !executable.is_file() {
        return Err(format!(
            "Bundled local API sidecar is missing: {}",
            executable.display()
        ));
    }

    let mut child = Command::new(executable)
        .env("PDF_EDITOR_OFFLINE_API_HOST", "127.0.0.1")
        .env("PDF_EDITOR_OFFLINE_API_PORT", port.to_string())
        .env("PDF_EDITOR_OFFLINE_API_TOKEN", api_token)
        .env("PDF_EDITOR_OFFLINE_STORAGE_DIR", storage_dir)
        .env("PDF_EDITOR_OFFLINE_TEMP_DIR", temp_dir)
        .env(
            "CORS_ORIGINS",
            "http://localhost,http://127.0.0.1,http://localhost:3000,http://127.0.0.1:3000,tauri://localhost,http://tauri.localhost",
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                println!("[pdf-editor-offline-api] {line}");
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("[pdf-editor-offline-api] {line}");
            }
        });
    }

    Ok(child)
}

#[tauri::command]
fn get_api_connection(state: State<'_, DesktopState>) -> DesktopApiConnection {
    DesktopApiConnection {
        base_url: state.api_base_url.clone(),
        token: state.api_token.clone(),
    }
}

#[tauri::command]
fn open_pdf_file() -> Result<Option<DesktopFilePayload>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PDF documents", &["pdf"])
        .pick_file()
    else {
        return Ok(None);
    };

    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("document.pdf")
        .to_string();

    Ok(Some(DesktopFilePayload {
        name,
        size: bytes.len() as u64,
        bytes,
    }))
}

#[tauri::command]
fn save_file(default_filename: String, bytes: Vec<u8>) -> Result<bool, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_filename)
        .save_file()
    else {
        return Ok(false);
    };

    fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
fn recent_files_get(state: State<'_, DesktopState>) -> Vec<RecentFile> {
    read_recent_files(&state.recent_files_path)
}

#[tauri::command]
fn recent_files_add(state: State<'_, DesktopState>, file: RecentFile) -> Result<(), String> {
    let mut files = read_recent_files(&state.recent_files_path);
    files.retain(|item| item.name != file.name);
    files.insert(0, file);
    files.truncate(10);
    write_recent_files(&state.recent_files_path, &files)
}

#[tauri::command]
fn recent_files_remove(state: State<'_, DesktopState>, file_name: String) -> Result<(), String> {
    let mut files = read_recent_files(&state.recent_files_path);
    files.retain(|item| item.name != file_name);
    write_recent_files(&state.recent_files_path, &files)
}

#[tauri::command]
fn recent_files_clear(state: State<'_, DesktopState>) -> Result<(), String> {
    write_recent_files(&state.recent_files_path, &[])
}

pub fn run() {
    let port = portpicker::pick_unused_port().expect("No available localhost port");
    let api_base_url = format!("http://127.0.0.1:{port}");
    let api_token = uuid::Uuid::new_v4().simple().to_string();

    let app = tauri::Builder::default()
        .setup({
            let api_base_url = api_base_url.clone();
            let api_token = api_token.clone();
            move |app| {
                let mut child = start_sidecar(app, port, &api_token)?;
                if let Err(error) = wait_for_backend(port) {
                    stop_sidecar(&mut child);
                    return Err(error.into());
                }

                let app_data_dir = app.path().app_data_dir().map_err(|error| {
                    format!("Could not resolve recent-files data directory: {error}")
                })?;
                fs::create_dir_all(&app_data_dir).map_err(|error| error.to_string())?;
                app.manage(DesktopState {
                    api_base_url,
                    api_token,
                    recent_files_path: app_data_dir.join("recent-files.json"),
                    sidecar: Mutex::new(Some(child)),
                });

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_api_connection,
            open_pdf_file,
            save_file,
            recent_files_get,
            recent_files_add,
            recent_files_remove,
            recent_files_clear
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tauri application");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(state) = app_handle.try_state::<DesktopState>() {
                if let Some(mut child) = state.sidecar.lock().expect("sidecar lock poisoned").take()
                {
                    stop_sidecar(&mut child);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{read_recent_files, write_recent_files, RecentFile};
    use std::fs;

    fn temp_recent_files_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "pdf-editor-offline-recent-{}.json",
            uuid::Uuid::new_v4().simple()
        ))
    }

    #[test]
    fn recent_files_round_trip_without_document_bytes() {
        let path = temp_recent_files_path();
        let entries = vec![RecentFile {
            name: "synthetic.pdf".to_string(),
            size: 42,
            path: None,
            last_opened: "2026-08-23T20:00:00Z".to_string(),
        }];

        write_recent_files(&path, &entries).expect("write recent files");
        let reloaded = read_recent_files(&path);
        fs::remove_file(&path).expect("remove test recent file");

        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].name, "synthetic.pdf");
        assert_eq!(reloaded[0].size, 42);
        assert!(reloaded[0].path.is_none());
    }

    #[test]
    fn malformed_recent_file_state_fails_closed_to_empty() {
        let path = temp_recent_files_path();
        fs::write(&path, b"not-json").expect("write malformed state");
        let reloaded = read_recent_files(&path);
        fs::remove_file(&path).expect("remove malformed state");

        assert!(reloaded.is_empty());
    }
}
