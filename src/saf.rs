use std::path::PathBuf;
use std::io::Read;
use std::sync::Mutex;

static PICKED_URI: Mutex<Option<String>> = Mutex::new(None);

#[cfg(target_os = "android")]
pub fn open_file_picker(app: &android_activity::AndroidApp) {
    let vm_ptr = app.vm_as_ptr();
    if vm_ptr.is_null() {
        log::error!("JavaVM pointer is null");
        return;
    }

    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr.cast()).expect("Failed to create JavaVM");
        let mut env = vm.attach_current_thread().expect("Failed to attach thread");
        
        let ctx = ndk_context::android_context();
        let activity = env.new_local_ref(
            jni::objects::JObject::from_raw(ctx.context().cast())
        ).expect("Failed to get activity");

        env.call_method(
            &activity,
            "openFilePicker",
            "()V",
            &[],
        ).expect("Failed to call openFilePicker");
    }
}

pub fn check_picked_file() -> Option<String> {
    PICKED_URI.lock().ok()?.clone()
}

pub fn clear_picked_file() {
    if let Ok(mut guard) = PICKED_URI.lock() {
        *guard = None;
    }
}

#[no_mangle]
pub extern "C" fn Java_com_verso_ui_VersoActivity_nativeOnFilePicked(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    uri_string: jni::objects::JString,
) {
    let uri: String = env.get_string(&uri_string).unwrap().into();
    log::info!("📂 File picked: {}", uri);
    
    if let Ok(mut guard) = PICKED_URI.lock() {
        *guard = Some(uri);
    }
}

pub fn scan_for_games() -> Vec<(String, PathBuf, String)> {
    let mut games = Vec::new();
    let search_dirs = [
        "/storage/emulated/0/Download",
        "/storage/emulated/0",
        "/sdcard/Download",
    ];
    
    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if ext == "apk" || ext == "so" {
                        let name = path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        games.push((name, path, ext));
                    }
                }
            }
        }
    }
    games
}

pub fn extract_from_apk(apk_path: &std::path::Path) -> Option<Vec<u8>> {
    let apk_file = std::fs::File::open(apk_path).ok()?;
    let mut archive = zip::ZipArchive::new(apk_file).ok()?;
    
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if file.name().contains("libandengine.so") {
                let mut data = Vec::new();
                if file.read_to_end(&mut data).is_ok() && !data.is_empty() {
                    return Some(data);
                }
            }
        }
    }
    None
}

pub fn load_so_file(so_path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(so_path).ok().filter(|d| !d.is_empty())
}
