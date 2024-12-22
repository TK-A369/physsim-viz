mod utils;

use wasm_bindgen::prelude::*;
use web_sys;

use physsim;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, physsim-viz-rust!");
}

struct RunnerState {
    counter: i32,
}

#[wasm_bindgen]
pub struct Runner {
    closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
    token: i32,
}

#[wasm_bindgen]
impl Runner {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, wasm_bindgen::JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let canvas = document
            .get_element_by_id("physsim-viz-canvas")
            .expect("Canvas not found")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("Canvas isn't canvas");

        let ctx = canvas
            .get_context("webgl2")
            .expect("Couldn't get WebGL2 context")
            .unwrap()
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .unwrap();

        let vert_shader = compile_shader(
            &ctx,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            r##"#version 300 es

        in vec4 position;

        void main() {
            gl_Position = position;
        }
        "##,
        )?;

        let frag_shader = compile_shader(
            &ctx,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            r##"#version 300 es

        precision highp float;
        out vec4 outColor;

        void main() {
            outColor = vec4(1, 1, 1, 1);
        }
        "##,
        )?;

        let program = link_program(&ctx, &vert_shader, &frag_shader)?;
        ctx.use_program(Some(&program));

        web_sys::console::log_1(&("Initialized WebGL2!".into()));

        let pos_attrib_idx = ctx.get_attrib_location(&program, "position");
        let vbo = ctx.create_buffer().ok_or("Couldn't create VBO")?;
        ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

        let vao = ctx.create_vertex_array().ok_or("Couldn't create VAO")?;
        ctx.bind_vertex_array(Some(&vao));

        ctx.enable_vertex_attrib_array(pos_attrib_idx as u32);
        ctx.vertex_attrib_pointer_with_i32(
            pos_attrib_idx as u32,
            3,
            web_sys::WebGl2RenderingContext::FLOAT,
            false,
            0,
            0,
        );

        ctx.bind_vertex_array(Some(&vao));

        let mut runner_state = RunnerState { counter: 0 };

        let closure = Closure::new(move || {
            draw(&ctx, &vbo, &vao, &mut runner_state);
        });
        let token = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            100,
        )?;

        Ok(Runner { closure, token })
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        web_sys::console::log_1(&"Dropping Runner...".into());
        match web_sys::window() {
            Some(window) => window.clear_interval_with_handle(self.token),
            _ => {}
        }
    }
}

fn compile_shader(
    ctx: &web_sys::WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<web_sys::WebGlShader, String> {
    let shader = ctx
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Couldn't create shader object"))?;
    ctx.shader_source(&shader, source);
    ctx.compile_shader(&shader);

    if ctx
        .get_shader_parameter(&shader, web_sys::WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(ctx
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error occurred when creating shader")))
    }
}

fn link_program(
    ctx: &web_sys::WebGl2RenderingContext,
    vert_shader: &web_sys::WebGlShader,
    frag_shader: &web_sys::WebGlShader,
) -> Result<web_sys::WebGlProgram, String> {
    let program = ctx
        .create_program()
        .ok_or_else(|| String::from("Couldn't create program object"))?;

    ctx.attach_shader(&program, vert_shader);
    ctx.attach_shader(&program, frag_shader);
    ctx.link_program(&program);

    if ctx
        .get_program_parameter(&program, web_sys::WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(ctx
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error occurred when creating program")))
    }
}

fn draw(
    ctx: &web_sys::WebGl2RenderingContext,
    vbo: &web_sys::WebGlBuffer,
    vao: &web_sys::WebGlVertexArrayObject,
    state: &mut RunnerState,
) {
    web_sys::console::log_1(&"Drawing...".into());

    let vertices: [f32; 9] = [
        -0.7,
        -0.7,
        0.0,
        0.7,
        -0.7,
        0.0,
        -0.8 + (state.counter as f32) * 0.002,
        0.7,
        0.0,
    ];

    ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
    ctx.bind_vertex_array(Some(&vao));

    unsafe {
        let vertices_view = js_sys::Float32Array::view(&vertices);

        ctx.buffer_data_with_array_buffer_view(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            &vertices_view,
            web_sys::WebGl2RenderingContext::STATIC_DRAW,
        );
    }

    let vert_count = (vertices.len() / 3) as i32;

    ctx.clear_color(0.0, 0.0, 0.0, 1.0);
    ctx.clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);

    ctx.draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLES, 0, vert_count);

    state.counter += 1;
}
