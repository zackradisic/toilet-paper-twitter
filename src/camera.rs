use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, SquareMatrix};
use wgpu::util::DeviceExt;

use crate::{input::MovementState, OPENGL_TO_WGPU_MATRIX};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraRaw {
    matrix: [[f32; 4]; 4],
    dimensions: [f32; 2],
    scale: f32,
    // ugh
    _pad: f32,
}

pub struct Camera {
    pub matrix: Matrix4<f32>,

    buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub translate: cgmath::Vector3<f32>,
    pub scale: f32,
    pub height: f32,
    pub width: f32,
}

impl Camera {
    pub fn make_matrix(
        width: f32,
        height: f32,
        translate: &cgmath::Matrix4<f32>,
        scale: &cgmath::Matrix4<f32>,
    ) -> cgmath::Matrix4<f32> {
        // OPENGL_TO_WGPU_MATRIX
        //     * cgmath::ortho(
        //         -width / 2.0,
        //         width / 2.0,
        //         -height / 2.0,
        //         height / 2.0,
        //         0.1,
        //         100.0,
        //     )
        //     * translate.invert().unwrap()
        //     * scale
        OPENGL_TO_WGPU_MATRIX
            * cgmath::perspective(cgmath::Deg(45.0), width / height, 0.1, 100.0)
            * cgmath::Matrix4::look_at_rh(
                // translate.invert().unwrap(),
                (0.0, 0.0, 30.0).into(),
                (5.0, 0.0, 0.0).into(),
                (0.0, 1.0, 0.0).into(),
            )
            * scale
    }

    pub fn update_scale(&mut self, queue: &wgpu::Queue, scale: f32) {
        self.scale = scale.clamp(0.01, 256.0);
        self.matrix = Self::make_matrix(
            self.width,
            self.height,
            &cgmath::Matrix4::from_translation(self.translate.clone()),
            &cgmath::Matrix4::from_scale(self.scale),
        );
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[Self::to_raw(
                self.matrix.clone(),
                self.width,
                self.height,
                self.scale,
            )]),
        );
    }

    pub fn update_translate(&mut self, queue: &wgpu::Queue, translate: cgmath::Vector3<f32>) {
        self.translate = translate;
        self.matrix = Self::make_matrix(
            self.width,
            self.height,
            &cgmath::Matrix4::from_translation(self.translate.clone()),
            &cgmath::Matrix4::from_scale(self.scale),
        );
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[Self::to_raw(
                self.matrix.clone(),
                self.width,
                self.height,
                self.scale,
            )]),
        );
    }

    pub fn update(&mut self, queue: &wgpu::Queue, movement: MovementState) {
        let mut translate = self.translate;
        if movement.contains(MovementState::W) {
            translate.z += 1.0;
            // translate.y += 1.0;
        }
        if movement.contains(MovementState::A) {
            translate.x -= 1.0;
        }
        if movement.contains(MovementState::S) {
            // translate.y -= 1.0;
            translate.z -= 1.0;
        }
        if movement.contains(MovementState::D) {
            translate.x += 1.0;
        }
        self.update_translate(&queue, translate);
    }

    pub fn new(
        translate: cgmath::Vector3<f32>,
        width: f32,
        height: f32,
        scale: f32,
        device: &wgpu::Device,
    ) -> (Self, wgpu::BindGroupLayout) {
        let view_proj = Self::make_matrix(
            width,
            height,
            &cgmath::Matrix4::from_translation(translate.clone()),
            &cgmath::Matrix4::from_scale(scale),
        );

        let camera_raw = Self::to_raw(view_proj.clone(), width, height, scale);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_raw]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
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

        (
            Self {
                translate,
                scale,
                height,
                width,
                matrix: view_proj,
                buffer: camera_buffer,
                bind_group: camera_bind_group,
            },
            camera_bind_group_layout,
        )
    }

    pub fn resize(&mut self, width: f32, height: f32, queue: &wgpu::Queue) {
        self.width = width;
        self.height = height;
        self.matrix = Self::make_matrix(
            width,
            height,
            &cgmath::Matrix4::from_translation(self.translate.clone()),
            &cgmath::Matrix4::from_scale(self.scale),
        );
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[Self::to_raw(self.matrix.clone(), width, height, self.scale)]),
        );
    }

    fn to_raw(matrix: Matrix4<f32>, width: f32, height: f32, scale: f32) -> CameraRaw {
        CameraRaw {
            matrix: matrix.into(),
            // dimensions: [width * SCREEN_SCALE, height * SCREEN_SCALE],
            dimensions: [width, height],
            scale,
            _pad: 0.0,
        }
    }
}
