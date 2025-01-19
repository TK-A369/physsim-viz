mod utils;

use wasm_bindgen::prelude::*;
use web_sys;

use physsim;

const DRAW_INTERVAL: f32 = 100.0;
const PHYSICS_INTERVAL: f32 = 10.0;

struct KeysPressed {
    w: bool,
    s: bool,
    a: bool,
    d: bool,
    q: bool,
    e: bool,
    i: bool,
    k: bool,
    j: bool,
    l: bool,
    u: bool,
    o: bool,
}

impl KeysPressed {
    fn new() -> Self {
        Self {
            w: false,
            s: false,
            a: false,
            d: false,
            q: false,
            e: false,
            i: false,
            k: false,
            j: false,
            l: false,
            u: false,
            o: false,
        }
    }
}

struct RunnerState {
    counter: i32,
    rigid_body: physsim::RigidBody<f32>,
    wireframe: bool,
    keys_pressed: KeysPressed,
    camera_pos: nalgebra::Vector3<f32>,
    camera_rot: nalgebra::Rotation3<f32>,
}

impl RunnerState {
    fn new() -> Self {
        Self {
            counter: 0,
            rigid_body: physsim::RigidBody {
                pos: nalgebra::Vector3::new(0.0, 0.0, 0.0),
                lin_vel: nalgebra::Vector3::new(0.1, 0.0, 0.0),
                rot_mat: nalgebra::Matrix3::identity(),
                ang_mom: nalgebra::Vector3::new(0.5, 0.0, 0.0),
                inv_ine: nalgebra::Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
            },
            wireframe: false,
            keys_pressed: KeysPressed::new(),
            camera_pos: nalgebra::Vector3::<f32>::zeros(),
            camera_rot: nalgebra::Rotation3::<f32>::identity(),
        }
    }
}

#[wasm_bindgen]
pub struct Runner {
    draw_interval_closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
    draw_interval_token: i32,
    physics_interval_closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
    physics_interval_token: i32,
    keydown_closure: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::KeyboardEvent)>,
    keyup_closure: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::KeyboardEvent)>,
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

        let vbo = ctx.create_buffer().ok_or("Couldn't create VBO")?;

        let vert_shader_plain = compile_shader(
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
        let frag_shader_plain = compile_shader(
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

        let program_plain = link_program(&ctx, &vert_shader_plain, &frag_shader_plain)?;
        ctx.use_program(Some(&program_plain));

        let vao_plain = ctx.create_vertex_array().ok_or("Couldn't create VAO")?;
        ctx.bind_vertex_array(Some(&vao_plain));
        ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

        let plain_pos_attrib_idx = ctx.get_attrib_location(&program_plain, "position");
        ctx.enable_vertex_attrib_array(plain_pos_attrib_idx as u32);
        ctx.vertex_attrib_pointer_with_i32(
            plain_pos_attrib_idx as u32,
            3,
            web_sys::WebGl2RenderingContext::FLOAT,
            false,
            3 * 4,
            0 * 4,
        );

        let vert_shader_colored = compile_shader(
            &ctx,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            r##"#version 300 es

            in vec3 position;
            in vec3 color;
            uniform mat4 projection;
            out vec3 fColor;

            void main() {
                gl_Position = projection * vec4(position, 1.0);
                fColor = color;
            }
            "##,
        )?;
        let frag_shader_colored = compile_shader(
            &ctx,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            r##"#version 300 es

            precision highp float;
            in vec3 fColor;
            out vec4 outColor;

            void main() {
                outColor = vec4(fColor, 1);
            }
            "##,
        )?;

        let program_colored = link_program(&ctx, &vert_shader_colored, &frag_shader_colored)?;
        ctx.use_program(Some(&program_colored));

        let vao_colored = ctx.create_vertex_array().ok_or("Couldn't create VAO")?;
        ctx.bind_vertex_array(Some(&vao_colored));
        ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

        let colored_pos_attrib_idx = ctx.get_attrib_location(&program_colored, "position");
        ctx.enable_vertex_attrib_array(colored_pos_attrib_idx as u32);
        ctx.vertex_attrib_pointer_with_i32(
            colored_pos_attrib_idx as u32,
            3,
            web_sys::WebGl2RenderingContext::FLOAT,
            false,
            6 * 4,
            0 * 4,
        );
        let colored_color_attrib_idx = ctx.get_attrib_location(&program_colored, "color");
        ctx.enable_vertex_attrib_array(colored_color_attrib_idx as u32);
        ctx.vertex_attrib_pointer_with_i32(
            colored_color_attrib_idx as u32,
            3,
            web_sys::WebGl2RenderingContext::FLOAT,
            false,
            6 * 4,
            3 * 4,
        );

        web_sys::console::log_1(&("Initialized WebGL2!".into()));

        let runner_state = std::sync::Arc::new(std::sync::RwLock::new(RunnerState::new()));

        let draw_interval_closure = {
            let runner_state = runner_state.clone();
            Closure::new(move || {
                draw(
                    &ctx,
                    &vbo,
                    &vao_plain,
                    &program_plain,
                    &vao_colored,
                    &program_colored,
                    runner_state.clone(),
                );
            })
        };
        let draw_interval_token = window.set_interval_with_callback_and_timeout_and_arguments_0(
            draw_interval_closure.as_ref().unchecked_ref(),
            DRAW_INTERVAL as i32,
        )?;

        let physics_interval_closure = {
            let runner_state = runner_state.clone();
            Closure::new(move || {
                physics_step(runner_state.clone());
            })
        };
        let physics_interval_token = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                physics_interval_closure.as_ref().unchecked_ref(),
                PHYSICS_INTERVAL as i32,
            )?;

        // Keypresses
        let keydown_closure = {
            let runner_state = runner_state.clone();
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                move |ev: web_sys::KeyboardEvent| {
                    web_sys::console::log_1(&format!("Got keydown event! {}", ev.code()).into());

                    let mut state_locked = runner_state.write().unwrap();
                    match ev.code().as_str() {
                        "KeyV" => state_locked.wireframe = !state_locked.wireframe,
                        "KeyW" => state_locked.keys_pressed.w = true,
                        "KeyS" => state_locked.keys_pressed.s = true,
                        "KeyA" => state_locked.keys_pressed.a = true,
                        "KeyD" => state_locked.keys_pressed.d = true,
                        "KeyQ" => state_locked.keys_pressed.q = true,
                        "KeyE" => state_locked.keys_pressed.e = true,
                        "KeyI" => state_locked.keys_pressed.i = true,
                        "KeyK" => state_locked.keys_pressed.k = true,
                        "KeyJ" => state_locked.keys_pressed.j = true,
                        "KeyL" => state_locked.keys_pressed.l = true,
                        "KeyU" => state_locked.keys_pressed.u = true,
                        "KeyO" => state_locked.keys_pressed.o = true,
                        _ => {}
                    }
                },
            )
        };
        document
            .add_event_listener_with_callback(&"keydown", keydown_closure.as_ref().unchecked_ref())
            .unwrap();
        let keyup_closure =
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                move |ev: web_sys::KeyboardEvent| {
                    web_sys::console::log_1(&format!("Got keyup event! {}", ev.code()).into());

                    let mut state_locked = runner_state.write().unwrap();
                    match ev.code().as_str() {
                        "KeyW" => state_locked.keys_pressed.w = false,
                        "KeyS" => state_locked.keys_pressed.s = false,
                        "KeyA" => state_locked.keys_pressed.a = false,
                        "KeyD" => state_locked.keys_pressed.d = false,
                        "KeyQ" => state_locked.keys_pressed.q = false,
                        "KeyE" => state_locked.keys_pressed.e = false,
                        "KeyI" => state_locked.keys_pressed.i = false,
                        "KeyK" => state_locked.keys_pressed.k = false,
                        "KeyJ" => state_locked.keys_pressed.j = false,
                        "KeyL" => state_locked.keys_pressed.l = false,
                        "KeyU" => state_locked.keys_pressed.u = false,
                        "KeyO" => state_locked.keys_pressed.o = false,
                        _ => {}
                    }
                },
            );
        document
            .add_event_listener_with_callback(&"keyup", keyup_closure.as_ref().unchecked_ref())
            .unwrap();

        Ok(Runner {
            draw_interval_closure,
            draw_interval_token,
            physics_interval_closure,
            physics_interval_token,
            keydown_closure,
            keyup_closure,
        })
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        web_sys::console::log_1(&"Dropping Runner...".into());
        match web_sys::window() {
            Some(window) => window.clear_interval_with_handle(self.draw_interval_token),
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

fn vector_to_vertices(
    vertices: &mut Vec<f32>,
    pos: &nalgebra::Vector3<f32>,
    v: &nalgebra::Vector3<f32>,
    color: Option<(f32, f32, f32)>,
    tip_size: f32,
) {
    fn add_vert(
        vertices: &mut Vec<f32>,
        v: &nalgebra::Vector3<f32>,
        color: Option<(f32, f32, f32)>,
    ) {
        vertices.push(v.x);
        vertices.push(v.y);
        vertices.push(v.z);
        if let Some(color) = color {
            vertices.push(color.0);
            vertices.push(color.1);
            vertices.push(color.2);
        }
    }

    // Main vector line
    add_vert(vertices, pos, color);
    add_vert(vertices, &(pos + v), color);

    // Vector tip
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(tip_size, 0.0, 0.0)),
        color,
    );
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(-tip_size, 0.0, 0.0)),
        color,
    );
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(0.0, tip_size, 0.0)),
        color,
    );
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(0.0, -tip_size, 0.0)),
        color,
    );
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(0.0, 0.0, tip_size)),
        color,
    );
    add_vert(vertices, &(pos + v), color);
    add_vert(
        vertices,
        &(pos + v + nalgebra::Vector3::new(0.0, 0.0, -tip_size)),
        color,
    );
}

fn draw(
    ctx: &web_sys::WebGl2RenderingContext,
    vbo: &web_sys::WebGlBuffer,
    vao_plain: &web_sys::WebGlVertexArrayObject,
    program_plain: &web_sys::WebGlProgram,
    vao_colored: &web_sys::WebGlVertexArrayObject,
    program_colored: &web_sys::WebGlProgram,
    state: std::sync::Arc<std::sync::RwLock<RunnerState>>,
) {
    web_sys::console::log_1(&"Drawing...".into());

    let state_locked = state.read().unwrap();

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

    let translation = nalgebra::Translation3::<f32>::from(state_locked.camera_pos);
    let proj_mat = persp.as_matrix()
        * (translation.to_homogeneous() * state_locked.camera_rot.to_homogeneous())
            .try_inverse()
            .unwrap();

    ctx.use_program(Some(program_plain));
    ctx.bind_vertex_array(Some(&vao_plain));
    //ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

    let plain_proj_uni_loc = ctx
        .get_uniform_location(program_plain, "projection")
        .expect("Uniform projection not found");
    ctx.uniform_matrix4fv_with_f32_array(
        Some(&plain_proj_uni_loc),
        false,
        &proj_mat.data.0.as_flattened(),
    );

    let mut vertices_plain: Vec<f32> = Vec::new();
    cuboid_to_vertices(
        &mut vertices_plain,
        &state_locked.rigid_body,
        state_locked.wireframe,
    );

    //unsafe {
    //    let vertices_view = js_sys::Float32Array::view(&vertices_plain);
    //
    //    ctx.buffer_data_with_array_buffer_view(
    //        web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
    //        &vertices_view,
    //        web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
    //    );
    //}
    let vertices_plain_f32_array =
        js_sys::Float32Array::new_with_length(vertices_plain.len() as u32);
    vertices_plain_f32_array.copy_from(&vertices_plain);
    ctx.buffer_data_with_array_buffer_view(
        web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
        &vertices_plain_f32_array,
        web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
    );

    let vert_count_plain = (vertices_plain.len() / 3) as i32;

    ctx.clear_color(0.0, 0.0, 0.0, 1.0);
    ctx.clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);

    if state_locked.wireframe {
        ctx.draw_arrays(web_sys::WebGl2RenderingContext::LINES, 0, vert_count_plain);
    } else {
        ctx.draw_arrays(
            web_sys::WebGl2RenderingContext::TRIANGLES,
            0,
            vert_count_plain,
        );
    }

    ctx.use_program(Some(program_colored));
    ctx.bind_vertex_array(Some(&vao_colored));
    //ctx.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

    let colored_proj_uni_loc = ctx
        .get_uniform_location(program_colored, "projection")
        .expect("Uniform projection not found");
    ctx.uniform_matrix4fv_with_f32_array(
        Some(&colored_proj_uni_loc),
        false,
        &proj_mat.data.0.as_flattened(),
    );

    let mut vertices_colored: Vec<f32> = Vec::new();

    let coords_system_axes_sizes = 5.0;
    vector_to_vertices(
        &mut vertices_colored,
        &nalgebra::Vector3::zeros(),
        &nalgebra::Vector3::new(coords_system_axes_sizes, 0.0, 0.0),
        Some((1.0, 0.0, 0.0)),
        0.5,
    );
    vector_to_vertices(
        &mut vertices_colored,
        &nalgebra::Vector3::zeros(),
        &nalgebra::Vector3::new(0.0, coords_system_axes_sizes, 0.0),
        Some((0.0, 1.0, 0.0)),
        0.5,
    );
    vector_to_vertices(
        &mut vertices_colored,
        &nalgebra::Vector3::zeros(),
        &nalgebra::Vector3::new(0.0, 0.0, coords_system_axes_sizes),
        Some((0.0, 0.0, 1.0)),
        0.5,
    );

    vector_to_vertices(
        &mut vertices_colored,
        &state_locked.rigid_body.pos,
        &state_locked.rigid_body.lin_vel,
        Some((1.0, 1.0, 0.0)),
        0.1,
    );
    vector_to_vertices(
        &mut vertices_colored,
        &state_locked.rigid_body.pos,
        &state_locked.rigid_body.ang_mom,
        Some((0.0, 1.0, 1.0)),
        0.1,
    );

    //unsafe {
    //    let vertices_view = js_sys::Float32Array::view(&vertices_colored);
    //
    //    ctx.buffer_data_with_array_buffer_view(
    //        web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
    //        &vertices_view,
    //        web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
    //    );
    //}
    let vertices_colored_f32_array =
        js_sys::Float32Array::new_with_length(vertices_colored.len() as u32);
    vertices_colored_f32_array.copy_from(&vertices_colored);
    ctx.buffer_data_with_array_buffer_view(
        web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
        &vertices_colored_f32_array,
        web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
    );

    let vert_count_colored = (vertices_colored.len() / 6) as i32;
    web_sys::console::log_1(&format!("vert_count_colored = {}", vertices_colored.len()).into());
    ctx.draw_arrays(
        web_sys::WebGl2RenderingContext::LINES,
        0,
        vert_count_colored,
    );
}

fn physics_step(state: std::sync::Arc<std::sync::RwLock<RunnerState>>) {
    let mut state_locked = state.write().unwrap();

    // Camera movement
    let cam_linear_speed: f32 = 0.001 * PHYSICS_INTERVAL;
    let cam_angular_sleep: f32 = 0.001 * PHYSICS_INTERVAL;
    if state_locked.keys_pressed.w {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(0.0, 0.0, -cam_linear_speed, 1.0)).rows(0, 3);
    }
    if state_locked.keys_pressed.s {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(0.0, 0.0, cam_linear_speed, 1.0)).rows(0, 3);
    }
    if state_locked.keys_pressed.a {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(-cam_linear_speed, 0.0, 0.0, 1.0)).rows(0, 3);
    }
    if state_locked.keys_pressed.d {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(cam_linear_speed, 0.0, 0.0, 1.0)).rows(0, 3);
    }
    if state_locked.keys_pressed.q {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(0.0, -cam_linear_speed, 0.0, 1.0)).rows(0, 3);
    }
    if state_locked.keys_pressed.e {
        let cam_rot_mat = state_locked.camera_rot.to_homogeneous();
        state_locked.camera_pos +=
            (cam_rot_mat * nalgebra::Vector4::new(0.0, cam_linear_speed, 0.0, 1.0)).rows(0, 3);
    }

    if state_locked.keys_pressed.i {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(cam_angular_sleep, 0.0, 0.0));
    }
    if state_locked.keys_pressed.k {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(-cam_angular_sleep, 0.0, 0.0));
    }
    if state_locked.keys_pressed.j {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(0.0, cam_angular_sleep, 0.0));
    }
    if state_locked.keys_pressed.l {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(0.0, -cam_angular_sleep, 0.0));
    }
    if state_locked.keys_pressed.u {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(0.0, 0.0, cam_angular_sleep));
    }
    if state_locked.keys_pressed.o {
        state_locked.camera_rot = state_locked.camera_rot
            * nalgebra::Rotation3::<f32>::new(nalgebra::Vector3::new(0.0, 0.0, -cam_angular_sleep));
    }

    state_locked.counter += 1;
    state_locked.rigid_body.step_sim(PHYSICS_INTERVAL / 1000.0);
}
