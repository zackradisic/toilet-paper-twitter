use cgmath::{vec4, Rotation3, Vector4};
use log::info;
use winit::{
    event::{
        DeviceEvent, ElementState, KeyboardInput, ModifiersState, MouseButton, VirtualKeyCode,
        WindowEvent,
    },
    window::Window,
};

use crate::{
    camera::Camera,
    cloth::Physics,
    convert_to_srgba,
    debug::Debug,
    input::{DragKind, InputState, MovementState},
    mouse::Mouse,
    texture::Texture,
    ColorGenerator, SAMPLE_COUNT, SCREEN_SCALE,
};

pub struct State {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub depth_texture: Texture,
    pub msaa_texture: Texture,

    pub physics: Physics,

    pub camera: Camera,

    #[cfg(feature = "debug")]
    pub debug: Debug,

    pub mouse: Mouse,
    pub input: InputState,
    pub bg: Vector4<f32>,
}

impl State {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    // features: wgpu::Features::DEPTH_CLIP_CONTROL,
                    // features: wgpu::Features::empty(),
                    features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits {
                            max_texture_dimension_2d: 4096,
                            ..wgpu::Limits::downlevel_webgl2_defaults()
                        }
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let color = ColorGenerator::new();

        let format = surface.get_supported_formats(&adapter)[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &config);

        let (w, h) = (800.0, 600.0);
        let (camera, camera_bind_group_layout) =
            Camera::new(cgmath::vec3(0.0, 0.0, 1.0), w, h, 1.0, &device);

        let depth_texture = Texture::create(
            &device,
            &config,
            Some(Texture::DEPTH_FORMAT),
            "Depth",
            SAMPLE_COUNT,
        );
        let msaa_texture = Texture::create(&device, &config, None, "MSAA", SAMPLE_COUNT);

        let bg = convert_to_srgba(vec4(20.0 / 256.0, 20.0 / 256., 28.0 / 256., 1.0));

        Self {
            physics: Physics::new(&device, &queue, format, &camera_bind_group_layout),
            surface,
            queue,
            config,
            size,
            depth_texture,
            msaa_texture,

            camera,

            #[cfg(feature = "debug")]
            debug: Debug::new(&device),

            bg,
            device,
            mouse: Mouse::default(),
            input: InputState::default(),
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => if let Some(pos) = &self.mouse.pos {},
                ElementState::Released => {}
            },
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y as f64,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y / 100.,
                };

                self.camera
                    .update_scale(&self.queue, self.camera.scale + y as f32);
            }
            WindowEvent::CursorLeft { .. } => {
                self.mouse.last_pos = self.mouse.pos.unwrap_or((0.0, 0.0).into());
                self.mouse.pos = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let mut vec: cgmath::Vector2<f32> = (position.x as f32, position.y as f32).into();
                vec.x -= self.camera.width / 2.0;
                vec.y -= self.camera.height / 2.0;
                // vec.x *= 2.0;
                vec.y *= -1.0;
                self.mouse.pos = Some(vec);
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: element_state,
                        virtual_keycode: Some(VirtualKeyCode::LShift),
                        ..
                    },
                ..
            } => {
                let pressed = matches!(element_state, ElementState::Pressed);
                self.input
                    .modifier_state
                    .set(ModifiersState::SHIFT, pressed);
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: element_state,
                        virtual_keycode: Some(VirtualKeyCode::LAlt),
                        ..
                    },
                ..
            } => {
                let pressed = matches!(element_state, ElementState::Pressed);
                self.input.modifier_state.set(ModifiersState::ALT, pressed);
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: element_state,
                        virtual_keycode: Some(key),
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::W => {
                    self.input.movement_state.set(
                        MovementState::W,
                        matches!(element_state, ElementState::Pressed),
                    );
                }
                VirtualKeyCode::A => {
                    self.input.movement_state.set(
                        MovementState::A,
                        matches!(element_state, ElementState::Pressed),
                    );
                }
                VirtualKeyCode::S => {
                    self.input.movement_state.set(
                        MovementState::S,
                        matches!(element_state, ElementState::Pressed),
                    );
                }
                VirtualKeyCode::D => {
                    self.input.movement_state.set(
                        MovementState::D,
                        matches!(element_state, ElementState::Pressed),
                    );
                }
                _ => (),
            },
            _ => (),
        }
        false
    }

    pub fn device_input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {}
            _ => (),
        }
        false
    }

    pub fn set_dragging(&mut self, dragging: Option<DragKind>) {
        self.input.dragging = dragging;
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Texture::create(
                &self.device,
                &self.config,
                Some(Texture::DEPTH_FORMAT),
                "Depth",
                SAMPLE_COUNT,
            );
            self.msaa_texture =
                Texture::create(&self.device, &self.config, None, "MSAA", SAMPLE_COUNT);
            self.camera
                .resize(new_size.width as f32, new_size.height as f32, &self.queue);
        };
    }

    pub fn update(&mut self) {
        self.physics.update(&self.queue);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let (view, resolve_target) = if SAMPLE_COUNT > 1 {
            (&self.msaa_texture.view, Some(&view))
        } else {
            (&view, None)
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.bg.x as f64,
                            g: self.bg.y as f64,
                            b: self.bg.z as f64,
                            a: self.bg.w as f64,
                        }),
                        store: true,
                    },
                })],
                // depth_stencil_attachment: None,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            self.physics
                .cloth
                .render(&self.camera.bind_group, &mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Debugging
        // wgpu::util::DownloadBuffer::read_buffer(
        //     &self.device,
        //     &self.queue,
        //     &self.debug.buffer.slice(0..1024),
        //     |result| {},
        // );

        Ok(())
    }
}
