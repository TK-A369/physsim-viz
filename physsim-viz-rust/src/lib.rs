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
    rigid_body: physsim::RigidBody<f32>,
    wireframe: bool,
}

impl RunnerState {
    fn new() -> Self {
        Self {
            counter: 0,
            rigid_body: physsim::RigidBody {
                pos: nalgebra::Vector3::new(0.0, 0.0, 0.0),
                lin_vel: nalgebra::Vector3::new(0.01, -0.02, 0.0),
                rot_mat: nalgebra::Matrix3::identity(),
                ang_mom: nalgebra::Vector3::new(0.5, -0.1, 0.3),
                inv_ine: nalgebra::Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
            },
            wireframe: false,
        }
    }
}

#[wasm_bindgen]
pub struct Runner {
    interval_closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
    keydown_closure: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::KeyboardEvent)>,
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

            in vec3 position;
            uniform mat4 projection;

            void main() {
                gl_Position = projection * vec4(position, 1.0);
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

        let mut runner_state = std::sync::Arc::new(std::sync::RwLock::new(RunnerState::new()));

        let interval_closure = {
            let runner_state = runner_state.clone();
            Closure::new(move || {
                draw(&ctx, &vbo, &vao, &program, runner_state.clone());
            })
        };
        let token = window.set_interval_with_callback_and_timeout_and_arguments_0(
            interval_closure.as_ref().unchecked_ref(),
            100,
        )?;

        // Keypresses
        let keydown_closure =
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                move |ev: web_sys::KeyboardEvent| {
                    web_sys::console::log_1(&format!("Got keydown event! {}", ev.code()).into());

                    if ev.code() == "KeyV" {
                        let mut state_locked = runner_state.write().unwrap();
                        state_locked.wireframe = !state_locked.wireframe;
                    }
                },
            );
        document
            .add_event_listener_with_callback(&"keydown", keydown_closure.as_ref().unchecked_ref())
            .unwrap();

        Ok(Runner {
            interval_closure,
            keydown_closure,
            token,
        })
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

fn cuboid_to_vertices(
    vertices: &mut Vec<f32>,
    rigid_body: &physsim::RigidBody<f32>,
    wireframe: bool,
) {
    fn add_vert(vertices: &mut Vec<f32>, v: nalgebra::Vector3<f32>) {
        vertices.push(v.x);
        vertices.push(v.y);
        vertices.push(v.z);
    }

    let v1 = rigid_body.rot_mat * nalgebra::Vector3::new(-0.5, -0.5, -0.5) + rigid_body.pos;
    let v2 = rigid_body.rot_mat * nalgebra::Vector3::new(-0.5, -0.5, 0.5) + rigid_body.pos;
    let v3 = rigid_body.rot_mat * nalgebra::Vector3::new(-0.5, 0.5, -0.5) + rigid_body.pos;
    let v4 = rigid_body.rot_mat * nalgebra::Vector3::new(-0.5, 0.5, 0.5) + rigid_body.pos;
    let v5 = rigid_body.rot_mat * nalgebra::Vector3::new(0.5, -0.5, -0.5) + rigid_body.pos;
    let v6 = rigid_body.rot_mat * nalgebra::Vector3::new(0.5, -0.5, 0.5) + rigid_body.pos;
    let v7 = rigid_body.rot_mat * nalgebra::Vector3::new(0.5, 0.5, -0.5) + rigid_body.pos;
    let v8 = rigid_body.rot_mat * nalgebra::Vector3::new(0.5, 0.5, 0.5) + rigid_body.pos;

    if wireframe {
        //E1
        add_vert(vertices, v1);
        add_vert(vertices, v2);

        //E2
        add_vert(vertices, v1);
        add_vert(vertices, v3);

        //E3
        add_vert(vertices, v3);
        add_vert(vertices, v4);

        //E4
        add_vert(vertices, v2);
        add_vert(vertices, v4);

        //E5
        add_vert(vertices, v5);
        add_vert(vertices, v6);

        //E6
        add_vert(vertices, v5);
        add_vert(vertices, v7);

        //E7
        add_vert(vertices, v7);
        add_vert(vertices, v8);

        //E8
        add_vert(vertices, v6);
        add_vert(vertices, v8);

        //E9
        add_vert(vertices, v1);
        add_vert(vertices, v5);

        //E10
        add_vert(vertices, v3);
        add_vert(vertices, v7);

        //E11
        add_vert(vertices, v4);
        add_vert(vertices, v8);

        //E12
        add_vert(vertices, v2);
        add_vert(vertices, v6);
    } else {
        //F1
        add_vert(vertices, v1);
        add_vert(vertices, v2);
        add_vert(vertices, v3);

        //F2
        add_vert(vertices, v2);
        add_vert(vertices, v3);
        add_vert(vertices, v4);

        //F3
        add_vert(vertices, v1);
        add_vert(vertices, v3);
        add_vert(vertices, v7);

        //F4
        add_vert(vertices, v1);
        add_vert(vertices, v5);
        add_vert(vertices, v7);

        //F5
        add_vert(vertices, v1);
        add_vert(vertices, v2);
        add_vert(vertices, v6);

        //F6
        add_vert(vertices, v1);
        add_vert(vertices, v5);
        add_vert(vertices, v6);

        //F7
        add_vert(vertices, v5);
        add_vert(vertices, v6);
        add_vert(vertices, v7);

        //F8
        add_vert(vertices, v6);
        add_vert(vertices, v7);
        add_vert(vertices, v8);

        //F9
        add_vert(vertices, v2);
        add_vert(vertices, v4);
        add_vert(vertices, v8);

        //F10
        add_vert(vertices, v2);
        add_vert(vertices, v6);
        add_vert(vertices, v8);

        //F11
        add_vert(vertices, v3);
        add_vert(vertices, v4);
        add_vert(vertices, v8);

        //F12
        add_vert(vertices, v3);
        add_vert(vertices, v7);
        add_vert(vertices, v8);
    }
}

fn draw(
    ctx: &web_sys::WebGl2RenderingContext,
    vbo: &web_sys::WebGlBuffer,
    vao: &web_sys::WebGlVertexArrayObject,
    program: &web_sys::WebGlProgram,
    state: std::sync::Arc<std::sync::RwLock<RunnerState>>,
) {
    web_sys::console::log_1(&"Drawing...".into());

    let mut state_locked = state.write().unwrap();

    web_sys::console::log_1(&format!("{:?}", state_locked.rigid_body).into());
    web_sys::console::log_1(&format!("{:?}", state_locked.rigid_body.rot_mat.determinant()).into());

    //let vertices: [f32; 9] = [
    //    -0.7,
    //    -0.7,
    //    0.0,
    //    0.7,
    //    -0.7,
    //    0.0,
    //    -0.8 + (state.counter as f32) * 0.002,
    //    0.7,
    //    0.0,
    //];

    let mut vertices: Vec<f32> = Vec::new();
    cuboid_to_vertices(
        &mut vertices,
        &state_locked.rigid_body,
        state_locked.wireframe,
    );

    let aspect = 1.333;
    let fovy: f32 = 75.0 * std::f32::consts::PI / 180.0;
    //let tan_half_fovy = (fovy / 2.0).tan();
    let z_far = 1000.0;
    let z_near = 0.01;

    //let mut proj_mat = nalgebra::Matrix4::<f32>::identity();
    //proj_mat.data.0[0][0] = 1.0 / (aspect * tan_half_fovy);
    //proj_mat.data.0[1][1] = 1.0 / tan_half_fovy;
    //proj_mat.data.0[2][2] = z_far / (z_far - z_near);
    //proj_mat.data.0[2][3] = 1.0;
    //proj_mat.data.0[3][2] = -(z_far * z_near) / (z_far - z_near);

    let persp = nalgebra::Perspective3::new(aspect, fovy, z_near, z_far);

    let translation = nalgebra::Translation3::<f32>::new(0.0, 0.0, -5.0);
    let proj_mat = persp.as_matrix() * translation.to_homogeneous();

    ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
    ctx.bind_vertex_array(Some(&vao));
    ctx.use_program(Some(program));

    let proj_uni_loc = ctx
        .get_uniform_location(program, "projection")
        .expect("Uniform projection not found");
    ctx.uniform_matrix4fv_with_f32_array(
        Some(&proj_uni_loc),
        false,
        &proj_mat.data.0.as_flattened(),
    );

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

    if state_locked.wireframe {
        ctx.draw_arrays(web_sys::WebGl2RenderingContext::LINES, 0, vert_count);
    } else {
        ctx.draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLES, 0, vert_count);
    }

    state_locked.counter += 1;
    state_locked.rigid_body.step_sim(0.1);
}
