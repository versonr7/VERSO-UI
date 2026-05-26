use std::rc::Rc;
use std::collections::VecDeque;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use glow::HasContext;
#[cfg(target_os = "android")]
use std::cell::Cell;

#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern "C" {}

// ─── عارض السجلات ───
struct LogViewer {
    messages: VecDeque<String>,
    max_lines: usize,
}

impl LogViewer {
    fn new(max_lines: usize) -> Self {
        LogViewer {
            messages: VecDeque::with_capacity(max_lines),
            max_lines,
        }
    }

    fn add(&mut self, msg: String) {
        if self.messages.len() >= self.max_lines {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    fn draw(&mut self, ui: &imgui::Ui) {
        ui.window("📋 Log Viewer")
            .size([600.0, 200.0], imgui::Condition::FirstUseEver)
            .build(|| {
                for msg in &self.messages {
                    ui.text_wrapped(msg);
                }
            });
    }
}

// ─── متصفح ملفات بسيط ───
struct FileBrowser {
    current_path: String,
    entries: Vec<String>,
}

impl FileBrowser {
    fn new(start_path: &str) -> Self {
        let mut fb = FileBrowser {
            current_path: start_path.to_string(),
            entries: Vec::new(),
        };
        fb.refresh();
        fb
    }

    fn refresh(&mut self) {
        self.entries.clear();
        self.entries.push("📁 ..".to_string());
        if let Ok(dir) = std::fs::read_dir(&self.current_path) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    self.entries.push(format!("📁 {}", name));
                } else if name.ends_with(".so") || name.ends_with(".apk") {
                    self.entries.push(format!("🎮 {}", name));
                } else {
                    self.entries.push(format!("📄 {}", name));
                }
            }
        }
    }

    fn draw(&mut self, ui: &imgui::Ui, on_select: &mut dyn FnMut(String)) {
        ui.window("📂 File Browser")
            .size([400.0, 400.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Path: {}", self.current_path));
                ui.separator();

                let mut selected_idx: Option<usize> = None;
                for (i, entry) in self.entries.iter().enumerate() {
                    if ui.selectable_config(entry).build() {
                        selected_idx = Some(i);
                    }
                }

                if let Some(idx) = selected_idx {
                    let entry = &self.entries[idx];
                    if entry == "📁 .." {
                        if let Some(parent) = std::path::Path::new(&self.current_path).parent() {
                            self.current_path = parent.to_string_lossy().to_string();
                            self.refresh();
                        }
                    } else if entry.starts_with("📁 ") {
                        let dir_name = &entry[6..];
                        let new_path = format!("{}/{}", self.current_path, dir_name);
                        self.current_path = new_path;
                        self.refresh();
                    } else if entry.starts_with("🎮 ") || entry.starts_with("📄 ") {
                        let file_name = entry[6..].to_string();
                        let full_path = format!("{}/{}", self.current_path, file_name);
                        on_select(full_path);
                    }
                }
            });
    }
}

#[cfg(target_os = "android")]
fn load_game_from_assets(app: &AndroidApp, emu: *mut std::ffi::c_void, log: &mut LogViewer) {
    let asset_manager = app.asset_manager();
    let filename = std::ffi::CString::new("libandengine.so").expect("CString failed");

    match asset_manager.open(&filename) {
        Some(mut asset) => {
            use std::io::Read;
            let mut elf_data = Vec::new();
            if asset.read_to_end(&mut elf_data).is_ok() && !elf_data.is_empty() {
                log.add(format!("✅ Loaded libandengine.so ({} bytes)", elf_data.len()));
                use thumb_arm::{emu_load_elf, emu_init_android};
                let entry = emu_load_elf(emu, elf_data.as_ptr(), elf_data.len() as u32);
                if entry > 0 {
                    log.add(format!("✅ ELF entry: 0x{:08X}", entry));
                    emu_init_android(emu);
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
fn load_game_from_path(emu: *mut std::ffi::c_void, path: &str, log: &mut LogViewer) {
    match std::fs::read(path) {
        Ok(data) => {
            log.add(format!("✅ Loaded file ({} bytes) from {}", data.len(), path));
            use thumb_arm::{emu_load_elf, emu_init_android};
            let entry = emu_load_elf(emu, data.as_ptr(), data.len() as u32);
            if entry > 0 {
                log.add(format!("✅ ELF entry: 0x{:08X}", entry));
                emu_init_android(emu);
                log.add("✅ Android lifecycle initialized".to_string());
            } else {
                log.add("❌ Failed to load ELF".to_string());
            }
        }
        Err(e) => {
            log.add(format!("❌ Failed to read file: {}", e));
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

    let mut log_viewer = LogViewer::new(200);
    log_viewer.add("🚀 VERSO-UI starting...".to_string());

    use thumb_arm::emu_create;
    let emu = emu_create();
    log_viewer.add("✅ Emulator created".to_string());

    load_game_from_assets(&app, emu, &mut log_viewer);

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
    // ✅ تعيين حجم الشاشة
    imgui.io_mut().display_size = [1920.0, 1080.0];
    log_viewer.add("✅ ImGui context created".to_string());

    let mut texture_map: imgui_glow_renderer::SimpleTextureMap = Default::default();
    let mut renderer = imgui_glow_renderer::Renderer::initialize(
        &gl,
        &mut imgui,
        &mut texture_map,
        false,
    ).expect("ImGui renderer init");
    log_viewer.add("✅ ImGui renderer initialized".to_string());

    let start_path = "/storage/emulated/0".to_string();
    let mut file_browser = FileBrowser::new(&start_path);
    log_viewer.add(format!("📂 File browser ready at {}", start_path));

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

        // ✅ تقليل خطوات المحاكاة لتجنب تجمد الواجهة
        use thumb_arm::{emu_step_batch, emu_get_pc};
        let steps = emu_step_batch(emu, 5000); // كان 10000، الآن 5000
        let pc = emu_get_pc(emu);

        let ui = imgui.new_frame();

        // القائمة الرئيسية
        ui.main_menu_bar(|| {
            ui.menu("View", || {
                ui.checkbox("Log Viewer", &mut show_log);
                ui.checkbox("File Browser", &mut show_browser);
            });
        });

        // نافذة معلومات المحاكي
        ui.window("🎮 Emulator Info")
            .size([300.0, 200.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("FPS: {:.1}", 1.0 / delta_s));
                ui.text(format!("PC: 0x{:08X}", pc));
                ui.text(format!("Steps/frame: {}", steps));
                ui.separator();
                if ui.button("⏸️ Pause") {
                    log_viewer.add("⏸️ Pause clicked".to_string());
                }
                ui.same_line();
                if ui.button("▶️ Resume") {
                    log_viewer.add("▶️ Resume clicked".to_string());
                }
            });

        // نافذة السجلات
        if show_log {
            log_viewer.draw(&ui);
        }

        // نافذة متصفح الملفات
        if show_browser {
            file_browser.draw(&ui, &mut |selected_path| {
                log_viewer.add(format!("🖱️ Selected: {}", selected_path));
                load_game_from_path(emu, &selected_path, &mut log_viewer);
            });
        }

        // رسم الخلفية
        unsafe {
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        // رسم ImGui
        let draw_data = imgui.render();
        renderer.render(&gl, &mut texture_map, draw_data).expect("ImGui render");

        egl.swap_buffers(display, surface).expect("swap_buffers");

        // معالجة أحداث النظام بسرعة لمنع "عدم الاستجابة"
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});
    }
}
