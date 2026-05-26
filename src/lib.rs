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
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("VersoUI"),
    );
    log::info!("=== ImGui Minimal Test ===");

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
    log::info!("Window acquired");

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

    // تهيئة Dear ImGui
    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let mut texture_map: imgui_glow_renderer::SimpleTextureMap = Default::default();
    let mut renderer = imgui_glow_renderer::Renderer::initialize(
        &gl,
        &mut imgui,
        &mut texture_map,
        false,
    ).expect("ImGui renderer init");

    log::info!("Entering main loop with ImGui");

    let mut last_time = std::time::Instant::now();
    loop {
        let now = std::time::Instant::now();
        let delta = now - last_time;
        last_time = now;
        let delta_s = delta.as_secs_f64();

        let io = imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f64(delta_s));

        let ui = imgui.new_frame();
        ui.window("VERSO-UI Test")
            .size([300.0, 150.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("FPS: {:.1}", 1.0 / delta_s));
                ui.text("ImGui is working!");
                if ui.button("Click me") {
                    log::info!("Button clicked");
                }
            });

        unsafe {
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let draw_data = imgui.render();
        if let Err(e) = renderer.render(&gl, &mut texture_map, draw_data) {
            log::error!("ImGui render error: {:?}", e);
        }

        egl.swap_buffers(display, surface).expect("swap_buffers");
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});
    }
}
