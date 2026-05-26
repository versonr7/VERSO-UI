use thumb_arm::{emu_create, emu_load_elf, emu_init_android, emu_step_batch, emu_get_pc};
use std::rc::Rc;

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
fn load_game_from_assets(app: &AndroidApp, emu: *mut std::ffi::c_void) {
    let asset_manager = app.asset_manager();
    let filename = std::ffi::CString::new("libandengine.so").expect("CString failed");
    
    match asset_manager.open(&filename) {
        Some(mut asset) => {
            use std::io::Read;
            let mut elf_data = Vec::new();
            if asset.read_to_end(&mut elf_data).is_ok() && !elf_data.is_empty() {
                log::info!("Loaded libandengine.so ({} bytes) from assets", elf_data.len());
                let entry = emu_load_elf(emu, elf_data.as_ptr(), elf_data.len() as u32);
                if entry > 0 {
                    log::info!("ELF loaded, entry: 0x{:08X}", entry);
                    emu_init_android(emu);
                } else {
                    log::error!("Failed to load ELF (entry = 0)");
                }
            } else {
                log::error!("Failed to read libandengine.so from assets");
            }
        }
        None => {
            log::warn!("libandengine.so not found in assets (this is OK for testing)");
        }
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("VersoUI"),
    );
    log::info!("Verso UI starting (Dear ImGui + glow)");

    let emu = emu_create();

    // تحميل اللعبة باستخدام AssetManager (الطريقة الصحيحة لأندرويد)
    load_game_from_assets(&app, emu);

    // انتظار النافذة الأصلية
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

    // تهيئة EGL و OpenGL ES
    use khronos_egl as egl;
    let egl = egl::Instance::new(egl::Static);
    let display = unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }.expect("get_display");
    egl.initialize(display).expect("eglInitialize");

    let config_attribs = [
        egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT as i32,
        egl::SURFACE_TYPE, egl::WINDOW_BIT as i32,
        egl::BLUE_SIZE, 8, egl::GREEN_SIZE, 8, egl::RED_SIZE, 8,
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

    let gl = Rc::new(unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name).map_or(std::ptr::null(), |addr| addr as *const _)
        })
    });
    log::info!("OpenGL context created");

    // 🎨 تهيئة Dear ImGui
    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    log::info!("ImGui context created");

    let mut texture_map: imgui_glow_renderer::SimpleTextureMap = Default::default();
    let mut renderer = imgui_glow_renderer::Renderer::initialize(
        &gl,
        &mut imgui,
        &mut texture_map,
        false,
    ).expect("فشل في تهيئة Renderer");
    log::info!("ImGui renderer initialized");

    // 🔁 الحلقة الرئيسية
    let mut last_time = std::time::Instant::now();
    let mut frame_count = 0u64;

    loop {
        let now = std::time::Instant::now();
        let delta = now - last_time;
        last_time = now;
        let delta_s = delta.as_secs_f64();

        let io = imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f64(delta_s));

        // تنفيذ المحاكي
        let steps = emu_step_batch(emu, 10000);
        let pc = emu_get_pc(emu);

        // بناء واجهة ImGui
        let ui = imgui.new_frame();
        ui.window("VERSO-UI")
            .size([400.0, 300.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("FPS: {:.1}", 1.0 / delta_s));
                ui.text(format!("PC: 0x{:08X}", pc));
                ui.text(format!("Steps: {}", steps));

                if ui.button("Pause") {
                    log::info!("Pause button clicked");
                }
                if ui.button("Resume") {
                    log::info!("Resume button clicked");
                }
            });

        // مسح الخلفية
        unsafe {
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let draw_data = imgui.render();
        renderer.render(&gl, &mut texture_map, draw_data).expect("فشل في رسم ImGui");

        egl.swap_buffers(display, surface).expect("swap_buffers");
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});

        frame_count += 1;
        if frame_count == 1 {
            log::info!("First frame rendered successfully!");
        }
    }
}
