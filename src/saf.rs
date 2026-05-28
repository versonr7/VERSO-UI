use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct FoundGame {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
}

static GAMES: Mutex<Vec<FoundGame>> = Mutex::new(vec![]);
static SCANNING: Mutex<bool> = Mutex::new(false);
static SCAN_DONE: Mutex<bool> = Mutex::new(false);
static SCAN_LOG: Mutex<Vec<String>> = Mutex::new(vec![]);

pub fn start_scan() {
    if *SCANNING.lock().unwrap() { return; }
    *SCANNING.lock().unwrap() = true;
    *SCAN_DONE.lock().unwrap() = false;
    SCAN_LOG.lock().unwrap().clear();

    std::thread::spawn(|| {
        let games = scan_games();
        *GAMES.lock().unwrap() = games;
        *SCANNING.lock().unwrap() = false;
        *SCAN_DONE.lock().unwrap() = true;
    });
}

pub fn is_scanning() -> bool { *SCANNING.lock().unwrap() }
pub fn is_scan_done() -> bool {
    let mut done = SCAN_DONE.lock().unwrap();
    if *done { *done = false; true } else { false }
}
pub fn take_games() -> Vec<FoundGame> { std::mem::take(&mut *GAMES.lock().unwrap()) }
pub fn get_scan_log() -> Vec<String> { SCAN_LOG.lock().unwrap().clone() }

fn scan_games() -> Vec<FoundGame> {
    let mut games = Vec::new();
    let dirs = [
        "/storage/emulated/0/Download",
        "/storage/emulated/0",
        "/sdcard/Download",
        "/sdcard",
    ];

    for dir in &dirs {
        add_log(&format!("فحص: {}", dir));
        scan_dir_recursive(Path::new(dir), &mut games, 2);
    }
    add_log(&format!("✅ اكتمل: وجد {} لعبة", games.len()));
    games
}

fn scan_dir_recursive(dir: &Path, games: &mut Vec<FoundGame>, depth: u32) {
    if depth == 0 { return; }
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir_recursive(&path, games, depth - 1);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "apk" || ext_lower == "so" {
                        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        add_log(&format!("  وجد: {} ({})", file_name, ext_lower));
                        games.push(FoundGame {
                            name: path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                            path: path.clone(),
                            source: ext_lower,
                        });
                    }
                }
            }
        }
        Err(e) => add_log(&format!("  ❌ فشل: {}", e)),
    }
}

fn add_log(msg: &str) { SCAN_LOG.lock().unwrap().push(msg.to_string()); }

pub fn load_game(path: &Path) -> Option<Vec<u8>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("apk") | Some("APK") => {
            let file = std::fs::File::open(path).ok()?;
            let mut archive = zip::ZipArchive::new(file).ok()?;
            for i in 0..archive.len() {
                if let Ok(mut f) = archive.by_index(i) {
                    if f.name().contains("libandengine.so") {
                        let mut data = Vec::new();
                        f.read_to_end(&mut data).ok()?;
                        return Some(data);
                    }
                }
            }
            None
        }
        Some("so") | Some("SO") => std::fs::read(path).ok(),
        _ => None,
    }
}
