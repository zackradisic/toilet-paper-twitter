use cgmath::{vec3, vec4, InnerSpace, Rotation3, SquareMatrix, Transform, Vector4};
use log::info;
use wgpu::util::DeviceExt;
use winit::{
    event::{
        DeviceEvent, ElementState, KeyboardInput, ModifiersState, MouseButton, VirtualKeyCode,
        WindowEvent,
    },
    window::Window,
};

use crate::{
    camera::{self, Camera, CameraController, CameraUniform, Projection},
    cloth::Physics,
    convert_to_srgba,
    debug::Debug,
    input::{DragKind, InputState, MovementState},
    memo::Memoized,
    mouse::Mouse,
    ray::{Ray, RayPipeline},
    screen_space_to_clip_space,
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
    pub camera_controller: Memoized<CameraController>,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub projection: Projection,

    // pub ray_pipeline: RayPipeline,
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

        // let camera = Camera::new((0.0, 0.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let camera = Camera::new((0.0, 0.0, 00.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection =
            camera::Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 100.0);
        let camera_controller = CameraController::new(4.0 * 2.0, 4.0 * 3.0);
        let mut camera_uniform = CameraUniform::new(config.width as f32, config.height as f32);
        camera_uniform.update_view_proj(&camera, &projection);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let depth_texture = Texture::create_depth_texture(&device, &config, SAMPLE_COUNT, "Depth");
        let msaa_texture = Texture::create(&device, &config, None, "MSAA", SAMPLE_COUNT);

        let bg = convert_to_srgba(vec4(20.0 / 256.0, 20.0 / 256., 28.0 / 256., 1.0));
        // let bg = convert_to_srgba(vec4(255.0 / 256.0, 255.0 / 256., 255.0 / 256., 1.0));

        let ray_pipeline = RayPipeline::new(
            &device,
            &queue,
            config.format,
            &camera,
            &projection,
            &camera_bind_group_layout,
            &config,
        );

        Self {
            physics: Physics::new(&device, &queue, format, &camera_bind_group_layout),
            surface,
            queue,
            config,
            size,
            depth_texture,
            msaa_texture,

            camera,
            camera_controller: camera_controller.into(),
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            projection,

            // ray_pipeline,
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
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.movement_state.set(
                    MovementState::MOUSE_PRESSED,
                    *state == ElementState::Pressed,
                );
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::CursorLeft { .. } => {
                self.mouse.last_pos = self.mouse.pos.unwrap_or((0.0, 0.0).into());
                self.mouse.pos = None;
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let vec: cgmath::Vector2<f32> = (
                    position.x as f32 / SCREEN_SCALE,
                    position.y as f32 / SCREEN_SCALE,
                )
                    .into();
                // vec.x -= self.config.width as f32 / 2.0;
                // vec.y -= self.config.height as f32 / 2.0;
                // vec.x *= 2.0;
                // vec.y *= -1.0;
                self.mouse.pos = Some(vec);
                true
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
                true
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
                true
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: element_state,
                        virtual_keycode: Some(key),
                        ..
                    },
                ..
            } => {
                if self
                    .camera_controller
                    .process_keyboard(*key, *element_state)
                {
                    return true;
                }

                match key {
                    VirtualKeyCode::W => {
                        self.input.movement_state.set(
                            MovementState::W,
                            matches!(element_state, ElementState::Pressed),
                        );
                        true
                    }
                    VirtualKeyCode::A => {
                        self.input.movement_state.set(
                            MovementState::A,
                            matches!(element_state, ElementState::Pressed),
                        );
                        true
                    }
                    VirtualKeyCode::S => {
                        self.input.movement_state.set(
                            MovementState::S,
                            matches!(element_state, ElementState::Pressed),
                        );
                        true
                    }
                    VirtualKeyCode::D => {
                        self.input.movement_state.set(
                            MovementState::D,
                            matches!(element_state, ElementState::Pressed),
                        );
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn device_input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            // DeviceEvent::Key(KeyboardInput {
            //     virtual_keycode: Some(VirtualKeycode::),
            //     ..
            // }) => {}
            DeviceEvent::MouseMotion { delta } => {
                if self
                    .input
                    .movement_state
                    .contains(MovementState::MOUSE_PRESSED)
                {
                    if let Some(DragKind::Particle(x, y)) = self.input.dragging.as_ref() {
                        let dx = delta.0 as f32 * 2.0;
                        let dy = -delta.1 as f32 * 2.0;

                        self.physics.cloth.mouse_force(*x, *y, dx, dy);
                        self.physics.cloth.mouse_force(*x - 1, *y - 1, dx, dy);
                        self.physics.cloth.mouse_force(*x + 1, *y + 1, dx, dy);
                    } else {
                        self.camera_controller.process_mouse(delta.0, delta.1);
                    }
                }
            }
            DeviceEvent::Button { state, .. } => match state {
                ElementState::Pressed => {
                    self.input
                        .movement_state
                        .set(MovementState::MOUSE_PRESSED, true);

                    if !self.input.modifier_state.contains(ModifiersState::SHIFT) {
                        return false;
                    }

                    // let pos = self.mouse.pos.expect("Should be good");
                    let pos = screen_space_to_clip_space(
                        self.config.width as f32 / 2.0,
                        self.config.height as f32 / 2.0,
                        &self.mouse.pos.expect("Should be good"),
                    );
                    let inv_view = self.camera.calc_matrix().invert().unwrap();
                    let inv_proj = self.projection.calc_matrix().invert().unwrap();

                    let pos_near = inv_view * inv_proj * vec4(pos.x, pos.y, 0.1, 1.0);
                    let pos_near = pos_near.truncate() / pos_near.w;

                    let pos_far = inv_view * inv_proj * vec4(pos.x, pos.y, 100.0, 1.0);
                    let pos_far = pos_far.truncate() / pos_far.w;

                    let dir = (pos_near - pos_far).normalize();
                    let ray = Ray::new(
                        vec3(
                            self.camera.position.x,
                            self.camera.position.y,
                            self.camera.position.z,
                        ),
                        dir,
                    );
                    let hit = self.physics.cloth.intersects(&ray);
                    if let Some((x, y)) = hit {
                        println!("HIT: {:?}", hit);
                        self.input.dragging = Some(DragKind::Particle(x, y));
                        // self.physics.cloth.set_moveable(x, y, false);
                    }
                }
                ElementState::Released => {
                    self.input
                        .movement_state
                        .set(MovementState::MOUSE_PRESSED, false);

                    if let Some(DragKind::Particle(x, y)) = self.input.dragging {
                        // self.physics.cloth.set_moveable(x, y, true);
                        self.input.dragging = None;
                    }
                }
            },
            _ => (),
        }
        false
    }

    pub fn set_dragging(&mut self, dragging: Option<DragKind>) {
        self.input.dragging = dragging;
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        // UPDATED!
        if new_size.width > 0 && new_size.height > 0 {
            self.projection.resize(new_size.width, new_size.height);
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Texture::create_depth_texture(
                &self.device,
                &self.config,
                SAMPLE_COUNT,
                "depth_texture",
            );
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        // if let Some(mut camera_controller) = self.camera_controller.handle_updated() {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        // }

        // self.ray_pipeline
        //     .update(&self.queue, &self.camera, &self.projection, &self.config);
        self.physics.update(&self.queue, dt);
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
                .render(&self.camera_bind_group, &mut render_pass);

            // self.ray_pipeline
            //     .render(&mut render_pass, &self.camera_bind_group);
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
