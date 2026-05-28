use std::path::PathBuf;
use std::sync::Mutex;

static PICKED_FILE_DATA: Mutex<Option<Vec<u8>>> = Mutex::new(None);
static PICKED_FILE_NAME: Mutex<Option<String>> = Mutex::new(None);
static SAF_READY: Mutex<bool> = Mutex::new(false);

/// استدعاء SAF عبر JNI
#[cfg(target_os = "android")]
pub fn open_saf_picker(app: &android_activity::AndroidApp) {
    let vm_ptr = app.vm_as_ptr();
    if vm_ptr.is_null() { return; }

    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr.cast()).unwrap();
        let mut env = vm.attach_current_thread().unwrap();
        let ctx = ndk_context::android_context();
        let activity = env.new_local_ref(
            jni::objects::JObject::from_raw(ctx.context().cast())
        ).unwrap();

        env.call_method(&activity, "openFilePicker", "()V", &[]).unwrap();
    }
}

/// تحقق من وجود ملف تم اختياره عبر SAF
pub fn check_saf_result() -> Option<Vec<u8>> {
    if *SAF_READY.lock().unwrap() {
        *SAF_READY.lock().unwrap() = false;
        PICKED_FILE_DATA.lock().unwrap().take()
    } else {
        None
    }
}

/// تستدعيها Java عند استلام الملف
#[no_mangle]
pub extern "C" fn Java_com_verso_ui_VersoActivity_nativeOnFilePicked(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    uri_string: jni::objects::JString,
    file_data: jni::objects::JByteArray,
) {
    let uri: String = env.get_string(&uri_string).unwrap().into();
    let data = env.convert_byte_array(&file_data).unwrap();
    log::info!("📂 SAF picked: {} ({} bytes)", uri, data.len());
    *PICKED_FILE_NAME.lock().unwrap() = Some(uri);
    *PICKED_FILE_DATA.lock().unwrap() = Some(data);
    *SAF_READY.lock().unwrap() = true;
}
