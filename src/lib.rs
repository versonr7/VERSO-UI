use thumb_arm::emu_create;
use thumb_arm::emu_step_batch;
use thumb_arm::emu_get_pc;
use thumb_arm::emu_destroy;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("VersoUI"),
    );

    // إنشاء المحاكي عبر FFI
    let emu = emu_create();
    
    // تنفيذ خطوات
    let steps = unsafe { emu_step_batch(emu, 1000) };
    let pc = unsafe { emu_get_pc(emu) };
    
    log::info!("Verso UI - Executed {} steps, PC=0x{:08X}", steps, pc);
    
    // تنظيف
    unsafe { emu_destroy(emu); }
}
