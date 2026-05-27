use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use glow::HasContext;
#[cfg(target_os = "android")]
use std::cell::Cell;

#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern "C" {}

// 🎯 تغليف المؤشر الخام لجعله Send + Sync
pub struct EmuHandle(*mut std::ffi::c_void);
unsafe impl Send for EmuHandle {}
unsafe impl Sync for EmuHandle {}

#[cfg(target_os = "android")]
fn load_game_from_assets(app: &AndroidApp, emu: &EmuHandle) -> bool {
    let asset_manager = app.asset_manager();
    let filename = std::ffi::CString::new("libandengine.so").expect("CString failed");
    
    match asset_manager.open(&filename) {
        Some(mut asset) => {
            use std::io::Read;
            let mut elf_data = Vec::new();
            if asset.read_to_end(&mut elf_data).is_ok() && !elf_data.is_empty() {
                log::info!("Loaded libandengine.so ({} bytes)", elf_data.len());
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

    // ── 1. إنشاء المحاكي ──
    use thumb_arm::emu_create;
    let emu = EmuHandle(emu_create());
    log::info!("Emulator created");

    // ── 2. تحميل اللعبة ──
    let game_loaded = load_game_from_assets(&app, &emu);
    if game_loaded {
        log::info!("Flappy Bird loaded successfully!");
    } else {
        log::warn!("Flappy Bird not found in assets (app will run without game)");
    }

    // ── 3. تغليف المحاكي للخيوط ──
    let emu = Arc::new(Mutex::new(emu));
    let emu_clone = emu.clone();
    
    // ── 4. تشغيل المحاكي في خيط منفصل ──
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

    // ── 5. انتظار النافذة الأصلية ──
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

    // ── 6. تهيئة EGL و OpenGL ES ──
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

    // ── 7. تهيئة Dear ImGui ──
    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    imgui.io_mut().display_size = [screen_width, screen_height];

    let scale_factor = (screen_width / 1000.0).max(1.0);
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

    // ── 8. الحلقة الرئيسية ──
    loop {
        let now = std::time::Instant::now();
        let delta = now - last_time;
        last_time = now;
        let delta_s = delta.as_secs_f64();

        // جمع أحداث اللمس
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

        // تحديث ImGui IO
        let io = imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f64(delta_s));
        io.add_mouse_pos_event(mouse_pos);
        io.mouse_down[0] = mouse_down;

        // قراءة حالة المحاكي
        let pc = if game_loaded {
            let emu = emu.lock().unwrap();
            use thumb_arm::emu_get_pc;
            emu_get_pc(emu.0)
        } else {
            0
        };

        // بناء واجهة ImGui
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

        // رسم
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
