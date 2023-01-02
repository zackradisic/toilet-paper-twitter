use wgpu::{util::DeviceExt, ShaderStages};

const DEFAULT_INSTANCE_BUFFER_CAP: usize = 1024;

// #[derive(Debug)]
// #[repr(C)]
// pub enum DebugItem {
//     Index(u32),
//     Grid(DebugItemGrid),
// }

#[repr(C)]
#[derive(Debug)]
pub struct DebugItemGrid {
    pub tag: u32,
    _pad: u32,
    pub xy: [f32; 2],
    pub derivative: [f32; 2],
    pub grid: [f32; 2],
}

pub struct Debug {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Debug {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: &(0..(std::mem::size_of::<DebugItemGrid>() * DEFAULT_INSTANCE_BUFFER_CAP))
                .map(|_| 0u8)
                .collect::<Vec<_>>(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Debug bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Debug bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 1,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group,
            bind_group_layout,
        }
    }
}
