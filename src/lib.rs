use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Informacoes sobre um aplicativo instalado.
pub struct AppInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub bundle_id: Option<String>,
}

/// Retorna todos os diretórios .app de /Applications e ~/Applications.
pub fn get_installed_apps() -> Vec<PathBuf> {
    let mut apps = Vec::new();
    let dirs = vec![
        PathBuf::from("/Applications"),
        get_home().join("Applications"),
    ];

    for dir in dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    apps.push(path);
                }
            }
        }
    }

    apps.sort_by(|a, b| {
        a.file_stem()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .cmp(&b.file_stem().unwrap_or_default().to_ascii_lowercase())
    });
    apps
}

/// Retorna informacoes detalhadas de todos os apps instalados.
pub fn get_installed_app_infos() -> Vec<AppInfo> {
    get_installed_apps()
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = dir_size(&path).unwrap_or(0);
            let bundle_id = get_bundle_id(&path);
            AppInfo {
                name,
                path,
                size,
                bundle_id,
            }
        })
        .collect()
}

pub fn find_app(name: &str) -> Option<PathBuf> {
    let search_dirs = vec![
        PathBuf::from("/Applications"),
        get_home().join("Applications"),
    ];

    let app_filename = if name.ends_with(".app") {
        name.to_string()
    } else {
        format!("{}.app", name)
    };

    // Busca exata
    for dir in &search_dirs {
        let path = dir.join(&app_filename);
        if path.exists() {
            return Some(path);
        }
    }

    // Busca case-insensitive
    let name_lower = app_filename.to_lowercase();
    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_name = entry.file_name().to_string_lossy().to_lowercase();
                if entry_name == name_lower {
                    return Some(entry.path());
                }
            }
        }
    }

    None
}

pub fn get_bundle_id(app_path: &Path) -> Option<String> {
    let plist = app_path.join("Contents/Info.plist");
    if !plist.exists() {
        return None;
    }

    let output = Command::new("defaults")
        .args(["read", &plist.to_string_lossy(), "CFBundleIdentifier"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn find_related_files(app_name: &str, bundle_id: Option<&str>) -> Vec<PathBuf> {
    let home = get_home();
    let mut found = Vec::new();

    let search_dirs: Vec<PathBuf> = vec![
        home.join("Library/Application Support"),
        home.join("Library/Caches"),
        home.join("Library/Preferences"),
        home.join("Library/Logs"),
        home.join("Library/Containers"),
        home.join("Library/Group Containers"),
        home.join("Library/Saved Application State"),
        home.join("Library/WebKit"),
        home.join("Library/HTTPStorages"),
        home.join("Library/Cookies"),
    ];

    let mut search_terms: Vec<String> = vec![app_name.to_string()];
    if let Some(id) = bundle_id {
        search_terms.push(id.to_string());
    }

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_name = entry.file_name().to_string_lossy().to_string();
                for term in &search_terms {
                    if entry_name == *term
                        || entry_name.to_lowercase() == term.to_lowercase()
                        || entry_name.contains(term)
                        || entry_name
                            .to_lowercase()
                            .contains(&term.to_lowercase())
                    {
                        found.push(entry.path());
                        break;
                    }
                }
            }
        }
    }

    if let Some(id) = bundle_id {
        let pref_dir = home.join("Library/Preferences");
        let plist_file = pref_dir.join(format!("{}.plist", id));
        if plist_file.exists() && !found.contains(&plist_file) {
            found.push(plist_file);
        }
    }

    found.sort();
    found.dedup();
    found
}

pub fn is_app_running(app_name: &str) -> bool {
    let output = Command::new("pgrep")
        .args(["-f", &format!("{}.app", app_name)])
        .output();

    matches!(output, Ok(o) if o.status.success())
}

pub fn quit_app(app_name: &str) {
    let _ = Command::new("osascript")
        .args([
            "-e",
            &format!("tell application \"{}\" to quit", app_name),
        ])
        .output();
}

pub fn remove_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn dir_size(path: &Path) -> io::Result<u64> {
    let mut total: u64 = 0;
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += dir_size(&entry.path()).unwrap_or(0);
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn get_home() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string()))
}
