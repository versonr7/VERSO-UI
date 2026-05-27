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
    if *SCANNING.lock().unwrap() {
        return;
    }
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

pub fn is_scanning() -> bool {
    *SCANNING.lock().unwrap()
}

pub fn is_scan_done() -> bool {
    let mut done = SCAN_DONE.lock().unwrap();
    if *done {
        *done = false;
        true
    } else {
        false
    }
}

pub fn take_games() -> Vec<FoundGame> {
    let mut guard = GAMES.lock().unwrap();
    std::mem::take(&mut *guard)
}

pub fn get_scan_log() -> Vec<String> {
    SCAN_LOG.lock().unwrap().clone()
}

fn scan_games() -> Vec<FoundGame> {
    let mut games = Vec::new();
    let dirs = ["/storage/emulated/0/Download", "/storage/emulated/0", "/sdcard/Download"];
    
    for dir in &dirs {
        SCAN_LOG.lock().unwrap().push(format!("فحص: {}", dir));
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("apk") {
                            SCAN_LOG.lock().unwrap().push(format!("  وجد APK: {}", path.display()));
                            match std::fs::File::open(&path) {
                                Ok(file) => {
                                    match zip::ZipArchive::new(file) {
                                        Ok(mut archive) => {
                                            let mut found = false;
                                            for i in 0..archive.len() {
                                                if let Ok(mut f) = archive.by_index(i) {
                                                    if f.name().contains("libandengine.so") {
                                                        games.push(FoundGame {
                                                            name: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
                                                            path: path.clone(),
                                                            source: "apk".into(),
                                                        });
                                                        SCAN_LOG.lock().unwrap().push(format!("    ✅ يحتوي على libandengine.so"));
                                                        found = true;
                                                        break;
                                                    }
                                                }
                                            }
                                            if !found {
                                                SCAN_LOG.lock().unwrap().push(format!("    ❌ لا يحتوي على libandengine.so"));
                                            }
                                        }
                                        Err(e) => {
                                            SCAN_LOG.lock().unwrap().push(format!("    ❌ فشل فتح ZIP: {}", e));
                                        }
                                    }
                                }
                                Err(e) => {
                                    SCAN_LOG.lock().unwrap().push(format!("    ❌ فشل فتح الملف: {}", e));
                                }
                            }
                        } else if ext.eq_ignore_ascii_case("so") {
                            SCAN_LOG.lock().unwrap().push(format!("  وجد SO: {}", path.display()));
                            games.push(FoundGame {
                                name: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
                                path: path.clone(),
                                source: "so".into(),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                SCAN_LOG.lock().unwrap().push(format!("  ❌ فشل: {}", e));
            }
        }
    }
    SCAN_LOG.lock().unwrap().push(format!("✅ اكتمل: وجد {} لعبة", games.len()));
    games
}

pub fn extract_from_apk(apk_path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(apk_path).ok()?;
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

pub fn load_so_file(so_path: &Path) -> Option<Vec<u8>> {
    std::fs::read(so_path).ok()
}

pub fn load_game(path: &Path) -> Option<Vec<u8>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("apk") | Some("APK") => extract_from_apk(path),
        Some("so") | Some("SO") => load_so_file(path),
        _ => None,
    }
}
