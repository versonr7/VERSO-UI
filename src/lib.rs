use thumb_arm::emu_create;
use thumb_arm::emu_step_batch;
use thumb_arm::emu_get_pc;
use thumb_arm::emu_destroy;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use glow::HasContext;
#[cfg(target_os = "android")]
use std::cell::Cell;

#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern "C" {}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("VersoUI"),
    );
    log::info!("Verso UI starting (EGL + OpenGL + Thumb-ARM)");

    // ── 1. إنشاء المحاكي ──
    let emu = emu_create();
    log::info!("Emulator created");

    // ── 2. انتظار النافذة الأصلية ──
    let window_ready = Cell::new(false);
    let native_window = loop {
        app.poll_events(Some(std::time::Duration::from_millis(16)), |_event| {
            window_ready.set(true);
        });
        if window_ready.get() {
            if let Some(nw) = app.native_window() {
                break nw;
            }
        }
    };
    log::info!("Native window acquired");

    // ── 3. تهيئة EGL ──
    use khronos_egl as egl;
    let egl = egl::Instance::new(egl::Static);
    let display = unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }.expect("get_display");
    egl.initialize(display).expect("eglInitialize");

    let config_attribs = [
        egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT as i32,
        egl::SURFACE_TYPE, egl::WINDOW_BIT as i32,
        egl::BLUE_SIZE, 8,
        egl::GREEN_SIZE, 8,
        egl::RED_SIZE, 8,
        egl::NONE,
    ];
    let config = egl.choose_first_config(display, &config_attribs)
        .expect("choose_first_config")
        .expect("no config");

    let surface = unsafe {
        egl.create_window_surface(display, config, native_window.ptr().as_ptr() as *mut _, None)
    }.expect("create_window_surface");

    let context_attribs = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
    let context = egl.create_context(display, config, None, &context_attribs)
        .expect("create_context");
    egl.make_current(display, Some(surface), Some(surface), Some(context))
        .expect("make_current");
    log::info!("EGL initialized");

    // ── 4. إنشاء سياق glow (OpenGL) ──
    let gl = unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name).map_or(std::ptr::null(), |addr| addr as *const _)
        })
    };
    log::info!("OpenGL context created");

    // ── 5. الحلقة الرئيسية ──
    let mut frame_count = 0u64;
    let start_time = std::time::Instant::now();

    loop {
        // تنفيذ خطوات المحاكي
        let steps = emu_step_batch(emu, 10000);
        let pc = emu_get_pc(emu);

        // رسم الشاشة الحمراء (إثبات أن OpenGL يعمل)
        unsafe {
            gl.clear_color(1.0, 0.0, 0.0, 1.0); // أحمر
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        // تبديل المخازن المؤقتة
        egl.swap_buffers(display, surface).expect("swap_buffers");

        // معالجة الأحداث
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});

        // طباعة معلومات كل 60 إطارًا
        frame_count += 1;
        if frame_count % 60 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let fps = frame_count as f64 / elapsed;
            log::info!("FPS: {:.1}, Steps: {}, PC: 0x{:08X}", fps, steps, pc);
        }
    }

    // لن نصل هنا في التطبيق الحقيقي
    // emu_destroy(emu);
}
