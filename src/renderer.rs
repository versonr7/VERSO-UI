use glow::HasContext;

/// رسم مستطيل بلون خالص على الشاشة
pub fn draw_rectangle(
    gl: &glow::Context,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4], // RGBA
    screen_width: f32,
    screen_height: f32,
) {
    let x1 = (x / screen_width) * 2.0 - 1.0;
    let y1 = 1.0 - (y / screen_height) * 2.0;
    let x2 = ((x + width) / screen_width) * 2.0 - 1.0;
    let y2 = 1.0 - ((y + height) / screen_height) * 2.0;

    let vertices: [f32; 18] = [
        x1, y1, 0.0,
        x1, y2, 0.0,
        x2, y2, 0.0,
        x1, y1, 0.0,
        x2, y2, 0.0,
        x2, y1, 0.0,
    ];

    unsafe {
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, "
            attribute vec3 aPos;
            void main() { gl_Position = vec4(aPos, 1.0); }
        ");
        gl.compile_shader(vs);

        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, &format!(
            "void main() {{ gl_FragColor = vec4({}, {}, {}, {}); }}",
            color[0], color[1], color[2], color[3]
        ));
        gl.compile_shader(fs);

        let program = gl.create_program().unwrap();
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);

        let vao = gl.create_vertex_array().unwrap();
        let vbo = gl.create_buffer().unwrap();

        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&vertices),
            glow::STATIC_DRAW,
        );
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 0, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);

        gl.use_program(Some(program));
        gl.bind_vertex_array(Some(vao));
        gl.draw_arrays(glow::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);

        gl.delete_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        gl.delete_vertex_array(vao);
        gl.delete_buffer(vbo);
    }
}
