use std::path::{Path, PathBuf};
use std::io::Read;

pub struct FoundGame {
    pub path: PathBuf,
    pub name: String,
    pub source: String,
}

/// فتح متصفح ملفات Android الأصلي لاختيار ملف APK أو SO
#[cfg(target_os = "android")]
pub fn open_file_picker() -> Option<PathBuf> {
    // نحصل على JNIEnv من android-activity
    // هذا يتطلب استخدام ndk-glue أو android-activity مع دعم jni
    // سنستخدم طريقة مبسطة: نطلب من المستخدم إدخال المسار يدويًا
    None
}

pub fn scan_for_games() -> Vec<FoundGame> {
    let mut games = Vec::new();
    
    let search_dirs = [
        "/storage/emulated/0/Download",
        "/storage/emulated/0",
        "/sdcard/Download",
        "/sdcard",
    ];
    
    for dir in &search_dirs {
        log::info!("🔍 فحص المجلد: {}", dir);
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if ext == "apk" {
                            if let Ok(apk_file) = std::fs::File::open(&path) {
                                if let Ok(mut archive) = zip::ZipArchive::new(apk_file) {
                                    for i in 0..archive.len() {
                                        if let Ok(file) = archive.by_index(i) {
                                            if file.name().contains("libandengine.so") {
                                                let name = path.file_stem()
                                                    .unwrap_or_default()
                                                    .to_string_lossy()
                                                    .to_string();
                                                log::info!("🎮 لعبة مضافة: {}", name);
                                                games.push(FoundGame {
                                                    path: path.clone(),
                                                    name: name.clone(),
                                                    source: "apk".to_string(),
                                                });
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        } else if ext == "so" && path.file_name()
                            .map_or(false, |n| n.to_string_lossy().contains("libandengine")) 
                        {
                            let name = path.file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            log::info!("🎮 لعبة مضافة: {}", name);
                            games.push(FoundGame {
                                path: path.clone(),
                                name: name.clone(),
                                source: "so".to_string(),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("❌ فشل فحص المجلد {}: {}", dir, e);
            }
        }
    }
    
    log::info!("✅ تم العثور على {} لعبة", games.len());
    games
}

pub fn extract_from_apk(apk_path: &Path) -> Option<Vec<u8>> {
    let apk_file = std::fs::File::open(apk_path).ok()?;
    let mut archive = zip::ZipArchive::new(apk_file).ok()?;
    
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if file.name().contains("libandengine.so") {
                let mut data = Vec::new();
                if file.read_to_end(&mut data).is_ok() && !data.is_empty() {
                    log::info!("تم استخراج libandengine.so ({} بايت) من {}", data.len(), apk_path.display());
                    return Some(data);
                }
            }
        }
    }
    None
}

pub fn load_so_file(so_path: &Path) -> Option<Vec<u8>> {
    std::fs::read(so_path).ok().filter(|d| !d.is_empty())
}

pub fn load_game(path: &Path) -> Option<Vec<u8>> {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if ext == "apk" {
            return extract_from_apk(path);
        } else if ext == "so" {
            return load_so_file(path);
        }
    }
    None
}
