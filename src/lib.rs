mod ui;

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

// 🎯 تغليف المؤشر الخام في هيكل آمن
pub struct EmuHandle(*mut std::ffi::c_void);
unsafe impl Send for EmuHandle {}
unsafe impl Sync for EmuHandle {}

#[cfg(target_os = "android")]
fn load_game_from_assets(app: &AndroidApp, emu: &EmuHandle, log: &mut ui::log_viewer::LogViewer) {
    let asset_manager = app.asset_manager();
    let filename = std::ffi::CString::new("libandengine.so").expect("CString failed");
    match asset_manager.open(&filename) {
        Some(mut asset) => {
            use std::io::Read;
            let mut elf_data = Vec::new();
            if asset.read_to_end(&mut elf_data).is_ok() && !elf_data.is_empty() {
                log.add(format!("✅ Loaded libandengine.so ({} bytes)", elf_data.len()));
                use thumb_arm::{emu_load_elf, emu_init_android};
                let entry = emu_load_elf(emu.0, elf_data.as_ptr(), elf_data.len() as u32);
                if entry > 0 {
                    log.add(format!("✅ ELF entry: 0x{:08X}", entry));
                    emu_init_android(emu.0);
                    log.add("✅ Android lifecycle initialized".to_string());
                } else {
                    log.add("❌ Failed to load ELF (entry = 0)".to_string());
                }
            }
        }
        None => {
            log.add("⚠️ libandengine.so not found in assets".to_string());
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

    let mut log_viewer = ui::log_viewer::LogViewer::new(200);
    log_viewer.add("🚀 VERSO-UI starting...".to_string());

    use thumb_arm::emu_create;
    let emu = EmuHandle(emu_create());
    log_viewer.add("✅ Emulator created".to_string());

    load_game_from_assets(&app, &emu, &mut log_viewer);

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
    log_viewer.add("✅ Native window acquired".to_string());

    use khronos_egl as egl;
    let egl = egl::Instance::new(egl::Static);
    let display = unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }.expect("get_display");
    egl.initialize(display).expect("eglInitialize");
    log_viewer.add("✅ EGL initialized".to_string());

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
    log_viewer.add("✅ EGL fully initialized".to_string());

    let gl = Rc::new(unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name).map_or(std::ptr::null(), |addr| addr as *const _)
        })
    });
    log_viewer.add("✅ glow context created".to_string());

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let screen_width = native_window.width() as f32;
    let screen_height = native_window.height() as f32;
    imgui.io_mut().display_size = [screen_width, screen_height];
    log_viewer.add(format!("🖥️ Screen resolution: {}x{}", screen_width, screen_height));

    let mut texture_map: imgui_glow_renderer::SimpleTextureMap = Default::default();
    let mut renderer = imgui_glow_renderer::Renderer::initialize(
        &gl,
        &mut imgui,
        &mut texture_map,
        false,
    ).expect("ImGui renderer init");
    log_viewer.add("✅ ImGui renderer initialized".to_string());

    let start_path = "/storage/emulated/0".to_string();
    let mut file_browser = ui::file_browser::FileBrowser::new(&start_path);
    log_viewer.add(format!("📂 File browser ready at {}", start_path));

    // 🎯 تشغيل المحاكي في خيط منفصل مع EmuHandle الآمن
    let emu = Arc::new(Mutex::new(emu));
    let emu_clone = emu.clone();
    std::thread::spawn(move || {
        loop {
            let mut emu = emu_clone.lock().unwrap();
            use thumb_arm::emu_step_batch;
            emu_step_batch(emu.0, 1000);
        }
    });

    let mut last_time = std::time::Instant::now();
    let mut show_log = true;
    let mut show_browser = true;

    loop {
        let now = std::time::Instant::now();
        let delta = now - last_time;
        last_time = now;
        let delta_s = delta.as_secs_f64();

        let io = imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f64(delta_s));

        let emu = emu.lock().unwrap();
        use thumb_arm::emu_get_pc;
        let pc = emu_get_pc(emu.0);
        drop(emu);

        let ui = imgui.new_frame();

        ui.main_menu_bar(|| {
            ui.menu("View", || {
                ui.checkbox("Log Viewer", &mut show_log);
                ui.checkbox("File Browser", &mut show_browser);
            });
        });

        ui.window("🎮 Emulator Info")
            .size([300.0, 200.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("FPS: {:.1}", 1.0 / delta_s));
                ui.text(format!("PC: 0x{:08X}", pc));
                ui.separator();
                if ui.button("⏸️ Pause") {
                    log_viewer.add("⏸️ Pause clicked".to_string());
                }
                ui.same_line();
                if ui.button("▶️ Resume") {
                    log_viewer.add("▶️ Resume clicked".to_string());
                }
            });

        if show_log {
            log_viewer.draw(&ui);
        }

        if show_browser {
            file_browser.draw(&ui, &mut |selected_path| {
                log_viewer.add(format!("🖱️ Selected: {}", selected_path));
            });
        }

        unsafe {
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let draw_data = imgui.render();
        renderer.render(&gl, &mut texture_map, draw_data).expect("ImGui render");

        egl.swap_buffers(display, surface).expect("swap_buffers");
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});
    }
}
