use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// لعبة تم العثور عليها
pub struct FoundGame {
    pub name: String,
    pub path: PathBuf,
    pub source: String, // "apk" أو "so"
}

// نتائج البحث (مشتركة بين الخيطين)
static GAMES: Mutex<Vec<FoundGame>> = Mutex::new(vec![]);
static SCANNING: Mutex<bool> = Mutex::new(false);
static SCAN_DONE: Mutex<bool> = Mutex::new(false);
static SCAN_LOG: Mutex<Vec<String>> = Mutex::new(vec![]);

/// بدء الفحص في خيط منفصل
pub fn start_scan() {
    if *SCANNING.lock().unwrap() {
        return; // فحص جاري بالفعل
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

/// هل الفحص جارٍ حاليًا؟
pub fn is_scanning() -> bool {
    *SCANNING.lock().unwrap()
}

/// هل اكتمل الفحص؟ (يُعاد `true` مرة واحدة فقط)
pub fn is_scan_done() -> bool {
    let mut done = SCAN_DONE.lock().unwrap();
    if *done {
        *done = false; // إعادة التعيين بعد القراءة
        true
    } else {
        false
    }
}

/// استلام الألعاب التي تم العثور عليها
pub fn take_games() -> Vec<FoundGame> {
    let mut guard = GAMES.lock().unwrap();
    std::mem::take(&mut *guard)
}

/// الحصول على سجل الفحص (للتصحيح)
pub fn get_scan_log() -> Vec<String> {
    SCAN_LOG.lock().unwrap().clone()
}

/// فحص المجلدات عن ألعاب (تعمل في الخيط المنفصل)
fn scan_games() -> Vec<FoundGame> {
    let mut games = Vec::new();
    let dirs = [
        "/storage/emulated/0/Download",
        "/storage/emulated/0",
        "/sdcard/Download",
    ];

    for dir in &dirs {
        let dir_name = dir.to_string();
        add_log(&format!("فحص: {}", dir_name));

        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();

                    // الحصول على الامتداد
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();

                        if ext_lower == "apk" {
                            let file_name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "???".to_string());
                            add_log(&format!("  وجد APK: {}", file_name));

                            // فتح الملف والتحقق من محتواه
                            match std::fs::File::open(&path) {
                                Ok(file) => {
                                    match zip::ZipArchive::new(file) {
                                        Ok(mut archive) => {
                                            let mut found = false;
                                            for i in 0..archive.len() {
                                                if let Ok(mut f) = archive.by_index(i) {
                                                    if f.name().contains("libandengine.so") {
                                                        let name = path
                                                            .file_stem()
                                                            .map(|n| n.to_string_lossy().to_string())
                                                            .unwrap_or_else(|| "???".to_string());
                                                        games.push(FoundGame {
                                                            name,
                                                            path: path.clone(),
                                                            source: "apk".to_string(),
                                                        });
                                                        add_log("    ✅ يحتوي على libandengine.so");
                                                        found = true;
                                                        break;
                                                    }
                                                }
                                            }
                                            if !found {
                                                add_log("    ❌ لا يحتوي على libandengine.so");
                                            }
                                        }
                                        Err(e) => {
                                            add_log(&format!("    ❌ فشل فتح ZIP: {}", e));
                                        }
                                    }
                                }
                                Err(e) => {
                                    add_log(&format!("    ❌ فشل فتح الملف: {}", e));
                                }
                            }
                        } else if ext_lower == "so" {
                            let file_name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "???".to_string());
                            add_log(&format!("  وجد SO: {}", file_name));
                            let name = path
                                .file_stem()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "???".to_string());
                            games.push(FoundGame {
                                name,
                                path: path.clone(),
                                source: "so".to_string(),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                add_log(&format!("  ❌ فشل: {}", e));
            }
        }
    }

    add_log(&format!("✅ اكتمل: وجد {} لعبة", games.len()));
    games
}

/// إضافة رسالة إلى سجل الفحص
fn add_log(msg: &str) {
    SCAN_LOG.lock().unwrap().push(msg.to_string());
}

/// استخراج libandengine.so من ملف APK
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

/// تحميل ملف SO مباشرة
pub fn load_so_file(so_path: &Path) -> Option<Vec<u8>> {
    std::fs::read(so_path).ok()
}

/// تحميل اللعبة (استخراج من APK أو تحميل SO)
pub fn load_game(path: &Path) -> Option<Vec<u8>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("apk") | Some("APK") => extract_from_apk(path),
        Some("so") | Some("SO") => load_so_file(path),
        _ => None,
    }
}
