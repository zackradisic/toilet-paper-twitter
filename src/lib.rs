pub mod camera;
pub mod cloth;
pub mod input;
pub mod main_state;
pub mod memo;
pub mod mouse;
pub mod ray;
pub mod texture;

#[cfg(feature = "debug")]
pub mod debug;

use cfg_if::cfg_if;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use std::time::{SystemTime, UNIX_EPOCH};

use bytemuck::{Pod, Zeroable};
use cgmath::{vec2, ElementWise, Vector4};
use main_state::State;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub const SAMPLE_COUNT: u8 = 4;
    } else {
        pub const SAMPLE_COUNT: u8 = 4;
    }
}

cfg_if! {
    if #[cfg(target_os = "macos")] {
// For MacOS bc retina screens double the amount of pixels
        pub const SCREEN_SCALE: f32 = 2.0;
    } else {
        pub const SCREEN_SCALE: f32 = 1.0;
    }
}

pub fn convert_to_srgba(rgba: Vector4<f32>) -> Vector4<f32> {
    (rgba).add_element_wise(0.055).map(|val| val.powf(2.4))
    // (rgba).map(|val| val.powf(2.4))
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

pub const SAFE_FRAC_PI_2: f32 = std::f32::consts::FRAC_PI_2 - 0.0001;

pub fn screen_space_to_clip_space(
    width: f32,
    height: f32,
    pos: &cgmath::Vector2<f32>,
) -> cgmath::Vector2<f32> {
    // (0, 0) -> (1920, 1080)
    // (-960, -540) -> (960, 540)
    // (-1, -1) -> (1, 1)
    // let pos = cgmath::vec2(pos.x - (width), -(pos.y - (height)));
    // let pos = cgmath::vec2(pos.x / (width), pos.y / (height));
    // pos
    let screen_size = vec2(width, height);

    let mut ndc = pos.div_element_wise(screen_size) * 2.0 - vec2(1.0, 1.0);
    ndc.y = -ndc.y;
    ndc
}

pub fn clip_space_to_screen_space(
    width: f32,
    height: f32,
    pos: &cgmath::Vector2<f32>,
) -> cgmath::Vector2<f32> {
    let pos = cgmath::vec2(pos.x * width, pos.y * height);
    let pos = cgmath::vec2(pos.x + width, height - pos.y);
    pos
}

pub fn screen_vec_to_clip_vec(
    width: f32,
    height: f32,
    pos: &cgmath::Vector2<f32>,
) -> cgmath::Vector2<f32> {
    let pos = cgmath::vec2((2.0 * pos.x) / width, (2.0 * pos.y) / height);
    pos
}
pub fn clip_vec_to_screen_vec(
    width: f32,
    height: f32,
    pos: &cgmath::Vector2<f32>,
) -> cgmath::Vector2<f32> {
    let pos = cgmath::vec2((pos.x / 2.0) * width, (pos.y / 2.0) * height);
    pos
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex2 {
    pub position: [f32; 2],
}

impl Default for Vertex2 {
    fn default() -> Self {
        Self {
            position: Default::default(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    // _pad: f32,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Default::default(),
            // _pad: 0.0,
        }
    }
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0=>Float32x3];
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    let mut frame: u128 = 0;
    let mut start: u128 = 0;

    let event_loop = EventLoop::new();
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let window = WindowBuilder::new()
                .with_maximized(true)
                .with_resizable(true)
                .build(&event_loop)
                .unwrap();
        } else {
            let window = WindowBuilder::new()
                .with_inner_size(LogicalSize {
                    width: 800,
                    height: 600,
                })
                .with_resizable(true)
                .build(&event_loop)
                .unwrap();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    simple_logger::init_with_level(log::Level::Info).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info);
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        // Winit prevents sizing with CSS, so we have to set
        // the size manually when on web.
        use winit::dpi::PhysicalSize;
        // window.set_inner_size(PhysicalSize::new(1600, 1200));

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    let mut state = pollster::block_on(State::new(&window));
    let mut last_render_time = instant::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        #[cfg(not(target_arch = "wasm32"))]
        if start == 0 {
            start = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
        }

        match event {
            Event::DeviceEvent { event, .. } => {
                state.device_input(&event);
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // new_inner_size is &mut so w have to dereference it twice
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = instant::Instant::now();
                let dt = now - last_render_time;

                last_render_time = now;
                state.update(dt);

                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.resize(state.size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // We're ignoring timeouts
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
                frame += 1;

                #[cfg(not(target_arch = "wasm32"))]
                {
                    // std::thread::sleep(std::time::Duration::from_millis(100));
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    if now.as_millis() - start > 1000 {
                        let fps = frame as f64 / ((now.as_millis() - start) as f64 / 1000.0);
                        // window.set_title(&format!("{:.1$} fps", fps, 3));
                        window.set_title(&format!("{} fps — ", fps,));
                    }
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}

pub struct ColorGenerator {
    pub colors: Vec<Vector4<f32>>,
    pub idx: usize,
}

impl ColorGenerator {
    pub fn new() -> Self {
        Self {
            colors: vec![
                Self::hex_to_rgba("5FB49C"),
                Self::hex_to_rgba("F2B134"),
                Self::hex_to_rgba("F93943"),
                Self::hex_to_rgba("6EF9F5"),
                Self::hex_to_rgba("B33C86"),
                Self::hex_to_rgba("E4FF1A"),
                Self::hex_to_rgba("FFB800"),
                Self::hex_to_rgba("FF5714"),
                Self::hex_to_rgba("FFEECF"),
                Self::hex_to_rgba("4D9078"),
                Self::hex_to_rgba("D5F2E3"),
                Self::hex_to_rgba("FBF5F3"),
                Self::hex_to_rgba("C6CAED"),
                Self::hex_to_rgba("A288E3"),
                Self::hex_to_rgba("CCFFCB"),
            ],
            idx: 0,
        }
    }

    pub fn next(&mut self) -> Vector4<f32> {
        let idx = self.idx % self.colors.len();
        self.idx += 1;
        self.colors[idx].clone()
    }

    fn hex_to_rgba(hex: &str) -> Vector4<f32> {
        let mut hex = hex.to_string();
        if hex.len() == 3 {
            hex = format!(
                "{}{}{}{}{}{}",
                hex.chars().nth(0).unwrap(),
                hex.chars().nth(0).unwrap(),
                hex.chars().nth(1).unwrap(),
                hex.chars().nth(1).unwrap(),
                hex.chars().nth(2).unwrap(),
                hex.chars().nth(2).unwrap()
            );
        }
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
        // let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
        convert_to_srgba(Vector4::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            1.0,
        ))
    }
}

impl Iterator for ColorGenerator {
    type Item = Vector4<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next())
    }
}
