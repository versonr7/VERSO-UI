use std::path::Path;
use std::io::Read;

/// استخراج libandengine.so من APK
pub fn extract_from_apk(apk_path: &Path) -> Option<Vec<u8>> {
    let apk_file = std::fs::File::open(apk_path).ok()?;
    let mut archive = zip::ZipArchive::new(apk_file).ok()?;
    
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if file.name().contains("libandengine.so") {
                let mut data = Vec::new();
                if file.read_to_end(&mut data).is_ok() && !data.is_empty() {
                    log::info!("Extracted libandengine.so ({} bytes)", data.len());
                    return Some(data);
                }
            }
        }
    }
    None
}

/// تحميل ملف SO مباشرة
pub fn load_so_file(so_path: &Path) -> Option<Vec<u8>> {
    std::fs::read(so_path).ok().filter(|d| !d.is_empty())
}

/// تحميل اللعبة – استخراج من APK أو تحميل SO مباشرة
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
