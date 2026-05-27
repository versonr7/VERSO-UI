use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::io::Read;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use glow::HasContext;
#[cfg(target_os = "android")]
use std::cell::Cell;

#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern "C" {}

pub struct EmuHandle(*mut std::ffi::c_void);
unsafe impl Send for EmuHandle {}
unsafe impl Sync for EmuHandle {}

/// تحاول استخراج libandengine.so من ملفات APK في مجلد التحميلات
fn try_load_from_apk() -> Option<Vec<u8>> {
    let download_dir = "/storage/emulated/0/Download";
    if let Ok(entries) = std::fs::read_dir(download_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "apk").unwrap_or(false) {
                if let Ok(apk_file) = std::fs::File::open(&path) {
                    let mut archive = match zip::ZipArchive::new(apk_file) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    // محاولة العثور على libandengine.so داخل الـ APK
                    for i in 0..archive.len() {
                        if let Ok(mut file) = archive.by_index(i) {
                            if file.name().contains("libandengine.so") {
                                let mut data = Vec::new();
                                if file.read_to_end(&mut data).is_ok() && !data.is_empty() {
                                    log::info!("Extracted libandengine.so from APK: {}", path.display());
                                    return Some(data);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "android")]
fn load_game(emu: &EmuHandle) -> bool {
    // 1. محاولة التحميل من ملفات APK في التحميلات
    if let Some(data) = try_load_from_apk() {
        use thumb_arm::{emu_load_elf, emu_init_android};
        let entry = emu_load_elf(emu.0, data.as_ptr(), data.len() as u32);
        if entry > 0 {
            log::info!("Game loaded from APK, entry: 0x{:08X}", entry);
            emu_init_android(emu.0);
            return true;
        }
    }

    // 2. محاولة التحميل من أصول التطبيق (assets)
    // (سيتم استدعاء هذه الدالة من android_main حيث يمكننا الوصول إلى AssetManager)
    false
}

#[cfg(target_os = "android")]
fn load_game_from_assets(app: &AndroidApp, emu: &EmuHandle) -> bool {
    // أولاً نجرب APK
    if load_game(emu) {
        return true;
    }
    
    // ثم نجرب assets
    let asset_manager = app.asset_manager();
    let filename = std::ffi::CString::new("libandengine.so").expect("CString failed");
    
    match asset_manager.open(&filename) {
        Some(mut asset) => {
            let mut elf_data = Vec::new();
            if asset.read_to_end(&mut elf_data).is_ok() && !elf_data.is_empty() {
                log::info!("Loaded libandengine.so ({} bytes) from assets", elf_data.len());
                use thumb_arm::{emu_load_elf, emu_init_android};
                let entry = emu_load_elf(emu.0, elf_data.as_ptr(), elf_data.len() as u32);
                if entry > 0 {
                    log::info!("ELF entry: 0x{:08X}", entry);
                    emu_init_android(emu.0);
                    return true;
                }
            }
            false
        }
        None => false,
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("VersoUI"),
    );
    log::info!("=== Verso UI + Flappy Bird ===");

    use thumb_arm::emu_create;
    let emu = EmuHandle(emu_create());
    log::info!("Emulator created");

    let game_loaded = load_game_from_assets(&app, &emu);
    if game_loaded {
        log::info!("Flappy Bird loaded successfully!");
    } else {
        log::warn!("Flappy Bird not found");
    }

    let emu = Arc::new(Mutex::new(emu));
    let emu_clone = emu.clone();
    
    if game_loaded {
        std::thread::spawn(move || {
            loop {
                let mut emu = emu_clone.lock().unwrap();
                use thumb_arm::emu_step_batch;
                emu_step_batch(emu.0, 10000);
            }
        });
        log::info!("Emulator thread started");
    }

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

    let gl = Rc::new(unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name).map_or(std::ptr::null(), |addr| addr as *const _)
        })
    });

    let screen_width = native_window.width() as f32;
    let screen_height = native_window.height() as f32;

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    imgui.io_mut().display_size = [screen_width, screen_height];

    let scale_factor = 2.0;
    imgui.io_mut().font_global_scale = scale_factor;
    imgui.fonts().add_font(&[imgui::FontSource::DefaultFontData { config: Some(imgui::FontConfig {
        size_pixels: 24.0 * scale_factor,
        ..Default::default()
    })}]);

    let mut texture_map: imgui_glow_renderer::SimpleTextureMap = Default::default();
    let mut renderer = imgui_glow_renderer::Renderer::initialize(
        &gl, &mut imgui, &mut texture_map, false,
    ).expect("ImGui renderer init");

    let mut last_time = std::time::Instant::now();
    let mut mouse_pos: [f32; 2] = [0.0; 2];
    let mut mouse_down = false;

    loop {
        let now = std::time::Instant::now();
        let delta = now - last_time;
        last_time = now;
        let delta_s = delta.as_secs_f64();

        use android_activity::input::{InputEvent, MotionAction};
        use android_activity::InputStatus;

        app.input_events(|event| {
            match event {
                InputEvent::MotionEvent(motion) => {
                    if let Some(pointer) = motion.pointers().next() {
                        mouse_pos = [pointer.x() as f32, pointer.y() as f32];
                        match motion.action() {
                            MotionAction::Down | MotionAction::PointerDown => mouse_down = true,
                            MotionAction::Up | MotionAction::PointerUp => mouse_down = false,
                            _ => {}
                        }
                    }
                    InputStatus::Handled
                }
                InputEvent::KeyEvent(_) => InputStatus::Handled,
                _ => InputStatus::Unhandled,
            }
        });

        let io = imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f64(delta_s));
        io.add_mouse_pos_event(mouse_pos);
        io.mouse_down[0] = mouse_down;

        let pc = if game_loaded {
            let emu = emu.lock().unwrap();
            use thumb_arm::emu_get_pc;
            emu_get_pc(emu.0)
        } else {
            0
        };

        let ui = imgui.new_frame();
        ui.window("🎮 VERSO-UI")
            .size([700.0 * scale_factor, 500.0 * scale_factor], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("FPS: {:.1}", 1.0 / delta_s));
                ui.text(format!("Scale: {:.1}x", scale_factor));
                ui.separator();
                
                if game_loaded {
                    ui.text(format!("Game: ✅ Flappy Bird loaded"));
                    ui.text(format!("PC: 0x{:08X}", pc));
                } else {
                    ui.text("Game: ❌ Not loaded");
                    ui.text("Place an APK with libandengine.so in /Download");
                }
                
                ui.separator();
                
                if ui.button("▶️ Run") {
                    log::info!("Run button clicked");
                }
                ui.same_line();
                if ui.button("⏸️ Pause") {
                    log::info!("Pause button clicked");
                }
                ui.same_line();
                if ui.button("⏹️ Stop") {
                    log::info!("Stop button clicked");
                }
                
                ui.separator();
                ui.text("Status: Running...");
            });

        unsafe {
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let draw_data = imgui.render();
        renderer.render(&gl, &mut texture_map, draw_data).expect("ImGui render");

        egl.swap_buffers(display, surface).expect("swap_buffers");
        app.poll_events(Some(std::time::Duration::from_millis(0)), |_| {});
    }
}
