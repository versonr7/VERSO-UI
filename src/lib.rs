use thumb_arm::{emu_create, emu_load_elf, emu_init_android, emu_step_batch, emu_get_pc};

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
    log::info!("Verso UI starting");

    let emu = emu_create();

    // تحميل اللعبة
    if let Ok(data) = std::fs::read("assets/libandengine.so") {
        let entry = emu_load_elf(emu, data.as_ptr(), data.len() as u32);
        if entry > 0 {
            log::info!("ELF loaded, entry: 0x{:08X}", entry);
            emu_init_android(emu);
        }
    }

    // انتظار النافذة
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

    // تهيئة EGL
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

    let gl = unsafe {
        glow::Context::from_loader_function(|name| {
            egl.get_proc_address(name).map_or(std::ptr::null(), |addr| addr as *const _)
        })
    };

    // ══════════════════════════════════════════
    // 🟦 إعداد VAO, VBO, Shader
    // ══════════════════════════════════════════
    let vao = unsafe { gl.create_vertex_array().unwrap() };
    let vbo = unsafe { gl.create_buffer().unwrap() };

    // 6 رؤوس (مثلثين) → مستطيل
    let vertices: [f32; 18] = [
        -0.5,  0.5, 0.0,  // أعلى اليسار
        -0.5, -0.5, 0.0,  // أسفل اليسار
         0.5, -0.5, 0.0,  // أسفل اليمين
        -0.5,  0.5, 0.0,  // أعلى اليسار
         0.5, -0.5, 0.0,  // أسفل اليمين
         0.5,  0.5, 0.0,  // أعلى اليمين
    ];

    unsafe {
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&vertices),
            glow::STATIC_DRAW,
        );
        gl.vertex_attrib_pointer_f32(
            0, 3, glow::FLOAT, false, 0, 0,
        );
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
    }

    // Shader بسيط
    let vs = unsafe { gl.create_shader(glow::VERTEX_SHADER).unwrap() };
    let fs = unsafe { gl.create_shader(glow::FRAGMENT_SHADER).unwrap() };
    let program = unsafe { gl.create_program().unwrap() };

    unsafe {
        gl.shader_source(vs, "
            attribute vec3 aPos;
            void main() { gl_Position = vec4(aPos, 1.0); }
        ");
        gl.compile_shader(vs);

        gl.shader_source(fs, "
            void main() { gl_FragColor = vec4(0.0, 1.0, 0.0, 1.0); }
        ");
        gl.compile_shader(fs);

        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
    }

    // ══════════════════════════════════════════
    // 🔁 الحلقة الرئيسية
    // ══════════════════════════════════════════
    loop {
        emu_step_batch(emu, 10000);

        unsafe {
            gl.clear_color(0.0, 0.0, 0.0, 1.0); // خلفية سوداء
            gl.clear(glow::COLOR_BUFFER_BIT);

            // رسم المستطيل الأخضر
            gl.use_program(Some(program));
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);
            gl.bind_vertex_array(None);
        }

        egl.swap_buffers(display, surface).expect("swap_buffers");
        app.poll_events(Some(std::time::Duration::from_millis(1)), |_| {});
    }
}
