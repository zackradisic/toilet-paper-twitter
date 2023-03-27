use std::time::{SystemTime, UNIX_EPOCH};

use cgmath::{vec2, vec3, InnerSpace, Vector2, Vector3};
use wgpu::util::DeviceExt;

use crate::{texture::Texture, Vertex, Vertex2, SAMPLE_COUNT};

pub const TIME_STEP: f32 = 1.0 / 120.0;
pub const DT: f32 = 0.01;
pub const DAMPING: f32 = 0.01;
pub const DEFAULT_INSTANCE_BUFFER_COUNT: u64 = 1024;
pub const CONSTRAINT_ITERATIONS: usize = 30;

fn time_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

pub struct Physics {
    accumulator: f32,
    pub cloth: Cloth,
    current_time: f64,
}

impl Physics {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            current_time: time_secs(),
            accumulator: 0.0,
            cloth: Cloth::new(
                device,
                queue,
                format,
                camera_bind_group_layout,
                // more cloth-y toilet paper
                // 14.0,
                // 10.0,
                // 45,
                // 55,
                // most accurate toilet paper
                10.0,
                14.0,
                22,
                26,
                // long 16:9-like cloth
                // 10.0,
                // 14.0,
                // 45,
                // 55,
            ),
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: std::time::Duration) {
        // let new_time = time_secs();
        // let frame_time = new_time - self.current_time;
        // self.current_time = new_time;
        let frame_time = dt.as_secs_f64();

        let mut updated = false;

        self.accumulator += frame_time as f32;
        while self.accumulator >= TIME_STEP {
            self.accumulator -= TIME_STEP;
            self.cloth.update(TIME_STEP);
            updated = true;
        }

        if updated {
            self.cloth.update_normals();
            self.update_wgpu(&queue);
        }
    }

    pub fn update_wgpu(&mut self, queue: &wgpu::Queue) {
        self.cloth.update_wgpu(queue);
    }
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vector3<f32>,
    pub old_position: Vector3<f32>,
    pub acceleration: Vector3<f32>,
    pub tex_coords: Vector2<f32>,
    pub accumulated_normal: Vector3<f32>,
    pub is_movable: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0, 0.0).into(),
            old_position: (0.0, 0.0, 0.0).into(),
            acceleration: (0.0, 0.0, 0.0).into(),
            tex_coords: (0.0, 0.0).into(),
            accumulated_normal: (0.0, 0.0, 0.0).into(),
            is_movable: true,
        }
    }
}

impl Particle {
    pub fn add_normal(&mut self, normal: Vector3<f32>) {
        self.accumulated_normal += normal.normalize();
    }

    pub fn reset_normal(&mut self) {
        self.accumulated_normal = (0.0, 0.0, 0.0).into();
    }

    pub fn offset_pos(&mut self, offset: Vector3<f32>) {
        if self.is_movable {
            self.position += offset;
        }
    }

    pub fn make_unmovable(&mut self) {
        self.is_movable = false;
    }

    pub fn add_force(&mut self, dir: Vector3<f32>) {
        self.acceleration += dir;
    }

    pub fn time_step(&mut self, timestep: f32) {
        if self.is_movable {
            let temp = self.position;
            self.position = self.position
                + (self.position - self.old_position) * (1.0 - DAMPING)
                + self.acceleration * timestep;
            self.old_position = temp;
            self.acceleration = (0.0, 0.0, 0.0).into();
        }
    }
}

pub struct Constraint {
    pub p1: usize,
    pub p2: usize,
    pub rest_distance: f32,
}

impl Constraint {
    pub fn new(p1: usize, p2: usize, rest_distance: f32) -> Self {
        Self {
            p1,
            p2,
            rest_distance,
        }
    }

    pub fn satisfy(&self, particles: &mut [Particle]) {
        let p1_to_p2 = particles[self.p2].position - particles[self.p1].position;
        let current_distance = p1_to_p2.magnitude();
        let correction_half = p1_to_p2 * (1.0 - self.rest_distance / current_distance) * 0.5;
        particles[self.p1].offset_pos(correction_half);
        particles[self.p2].offset_pos(-correction_half);
    }
}

pub struct Cloth {
    pos: Vector3<f32>,
    old_pos: Vector3<f32>,
    acceleration: Vector3<f32>,
    particles: Vec<Particle>,
    constraints: Vec<Constraint>,

    num_particles_width: usize,
    num_particles_height: usize,

    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_normal_buffer: wgpu::Buffer,
    tex_coord_buffer: wgpu::Buffer,
    diffuse_bind_group: wgpu::BindGroup,
    texture: Texture,

    vertices: Vec<Vertex>,
    tex_coord: Vec<Vertex2>,
    normals: Vec<Vertex>,
}

impl Cloth {
    const INDICES: &[u16] = &[0, 2, 1];
    const NORMAL_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![1=>Float32x3];
    const TEX_COORD_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![2=>Float32x2];
    fn normal_desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::NORMAL_ATTRIBUTES,
        }
    }
    fn tex_coord_desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::TEX_COORD_ATTRIBUTES,
        }
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        width: f32,
        height: f32,
        num_particles_width: usize,
        num_particles_height: usize,
    ) -> Self {
        let mut particles: Vec<Particle> =
            vec![Default::default(); num_particles_height * num_particles_width];
        let mut constraints = vec![];

        let get_particle_idx = |x: usize, y: usize| -> usize { y * num_particles_width + x };
        fn make_constraint(
            p1: usize,
            p2: usize,
            particles: &mut [Particle],
            constraints: &mut Vec<Constraint>,
        ) {
            let p1_pos = particles[p1].position;
            let p2_pos = particles[p2].position;
            constraints.push(Constraint::new(p1, p2, (p1_pos - p2_pos).magnitude()));
        }

        // creating particles in a grid of particles from (0,0,0) to (width,-height,0)
        for x in 0..num_particles_width {
            for y in 0..num_particles_height {
                let pos = vec3(
                    width * (x as f32 / num_particles_width as f32),
                    -height * (y as f32 / num_particles_height as f32),
                    0.0,
                );

                // let idx = get_particle_idx(x, y);
                let idx = y * num_particles_width + x;
                particles[idx] = Particle {
                    position: pos.clone(),
                    old_position: pos,
                    acceleration: vec3(0.0, 0.0, 0.0),
                    accumulated_normal: vec3(0.0, 0.0, 0.0),
                    is_movable: true,

                    tex_coords: vec2(pos.x / width, pos.y.abs() / height),
                };
            }
        }

        // Connecting immediate neighbor particles with constraints (distance 1 and sqrt(2) in the grid)
        for x in 0..num_particles_width {
            for y in 0..num_particles_height {
                if x < num_particles_width - 1 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x + 1, y),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if y < num_particles_height - 1 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x, y + 1),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if x < num_particles_width - 1 && y < num_particles_height - 1 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x + 1, y + 1),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if x < num_particles_width - 1 && y < num_particles_height - 1 {
                    make_constraint(
                        get_particle_idx(x + 1, y),
                        get_particle_idx(x, y + 1),
                        &mut particles,
                        &mut constraints,
                    );
                }
            }
        }

        // Connecting secondary neighbors with constraints (distance 2 and sqrt(4) in the grid)
        for x in 0..num_particles_width {
            for y in 0..num_particles_height {
                if x < num_particles_width - 2 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x + 2, y),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if y < num_particles_height - 2 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x, y + 2),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if x < num_particles_width - 2 && y < num_particles_height - 2 {
                    make_constraint(
                        get_particle_idx(x, y),
                        get_particle_idx(x + 2, y + 2),
                        &mut particles,
                        &mut constraints,
                    );
                }
                if x < num_particles_width - 2 && y < num_particles_height - 2 {
                    make_constraint(
                        get_particle_idx(x + 2, y),
                        get_particle_idx(x, y + 2),
                        &mut particles,
                        &mut constraints,
                    );
                }
            }
        }

        for i in 0..3 {
            particles[get_particle_idx(i, 0)].offset_pos(vec3(0.5, 0.0, 0.0)); // moving the particle a bit towards the center, to make it hang more natural - because I like it ;)
            particles[get_particle_idx(i, 0)].make_unmovable();
            particles[get_particle_idx(i, 0)].offset_pos(vec3(-0.5, 0.0, 0.0)); // moving the particle a bit towards the center, to make it hang more natural - because I like it ;)
            particles[get_particle_idx(num_particles_width - 1 - i, 0)].make_unmovable();
        }

        let mut vertices = vec![];
        let mut normals = vec![];
        let mut tex_coord = vec![];

        let bytes = include_bytes!("tweet.png");
        let texture =
            Texture::from_bytes(device, queue, bytes, "tweet img").expect("To load image");

        let (pipeline, vertex_buffer, vertex_normal_buffer, tex_coord_buffer, diffuse_bind_group) =
            Self::create_render_pipeline(
                device,
                queue,
                format,
                &texture,
                camera_bind_group_layout,
                &mut vertices,
                &mut normals,
                &mut tex_coord,
                &particles,
                &constraints,
                num_particles_width,
                num_particles_height,
            );

        Self {
            particles,
            constraints,
            old_pos: (0.0, 0.0, 0.0).into(),
            pos: (0.0, 0.0, 0.0).into(),
            acceleration: (1.0, 1.0, 0.0).into(),

            num_particles_width,
            num_particles_height,

            pipeline,
            vertex_buffer,
            vertex_normal_buffer,
            tex_coord_buffer,
            diffuse_bind_group,
            texture,

            vertices,
            normals,
            tex_coord,
        }
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        texture: &Texture,
        camera_bind_group_layout: &wgpu::BindGroupLayout,

        vertices: &mut Vec<Vertex>,
        normals: &mut Vec<Vertex>,
        tex_coord: &mut Vec<Vertex2>,

        particles: &[Particle],
        constraints: &[Constraint],

        num_particles_width: usize,
        num_particles_height: usize,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::Buffer,
        wgpu::Buffer,
        wgpu::Buffer,
        wgpu::BindGroup,
    ) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("particle.wgsl").into()),
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &texture.sampler.as_ref().expect("Texture to have sampler"),
                    ),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle pipeline layout"),
            bind_group_layouts: &[camera_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Self::normal_desc(), Self::tex_coord_desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                // cull_mode: Some(wgpu::Face::Back),
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT as u32,
                ..Default::default()
            },
            multiview: None,
        });

        Self::fill_vertices(
            particles,
            vertices,
            normals,
            tex_coord,
            num_particles_width,
            num_particles_height,
        );
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let vertex_normal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: "Vertex Normal Buffer".into(),
            contents: bytemuck::cast_slice(&normals),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let tex_coord_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: "Texture Coord Buffer".into(),
            contents: bytemuck::cast_slice(&tex_coord),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        (
            pipeline,
            vertex_buffer,
            vertex_normal_buffer,
            tex_coord_buffer,
            diffuse_bind_group,
        )
    }

    fn calc_triangle_normal(p1: &Particle, p2: &Particle, p3: &Particle) -> Vector3<f32> {
        let pos1 = p1.position.clone();
        let pos2 = p2.position.clone();
        let pos3 = p3.position.clone();

        let v1 = pos2 - pos1;
        let v2 = pos3 - pos1;

        // v1.cross(v2).normalize()
        v1.cross(v2)
    }

    fn add_wind_forces_for_triangle(
        &mut self,
        p1i: usize,
        p2i: usize,
        p3i: usize,
        dir: Vector3<f32>,
    ) {
        let normal = Self::calc_triangle_normal(
            &self.particles[p1i],
            &self.particles[p2i],
            &self.particles[p3i],
        );

        let d = normal.normalize();
        let force = normal * d.dot(dir);
        self.particles[p1i].add_force(force);
        self.particles[p2i].add_force(force);
        self.particles[p3i].add_force(force);
    }

    fn fill_vertices(
        particles: &[Particle],
        vertices: &mut Vec<Vertex>,
        normals: &mut Vec<Vertex>,
        tex_coord: &mut Vec<Vertex2>,
        num_particles_width: usize,
        num_particles_height: usize,
    ) {
        vertices.clear();
        normals.clear();
        tex_coord.clear();

        let get_particle_idx = |x: usize, y: usize| -> usize { y * num_particles_width + x };

        for x in 0..num_particles_width - 1 {
            for y in 0..num_particles_height - 1 {
                let tmp = [
                    &particles[get_particle_idx(x + 1, y)],
                    &particles[get_particle_idx(x, y)],
                    &particles[get_particle_idx(x, y + 1)],
                    //
                    &particles[get_particle_idx(x + 1, y + 1)],
                    &particles[get_particle_idx(x + 1, y)],
                    &particles[get_particle_idx(x, y + 1)],
                ];

                vertices.extend(tmp.iter().map(|p| Vertex {
                    position: p.position.into(),
                    // _pad: 0.0,
                }));

                tex_coord.extend(tmp.iter().map(|p| Vertex2 {
                    position: p.tex_coords.into(),
                }));

                normals.extend(tmp.iter().map(|p| Vertex {
                    position: p.accumulated_normal.normalize().into(),
                    // _pad: 0.0,
                }));
            }
        }
    }

    pub fn update(&mut self, timestep: f32) {
        // gravity
        self.add_force(vec3(0.0, -2.8, 0.0) * timestep);
        self.add_wind_force(vec3(10.5, 0.0, 0.2) * timestep);
        // self.add_wind_force(vec3(100.5, 0.0, 0.2) * timestep);
        // self.add_wind_force(vec3(0.5, 0.0, 0.2) * timestep);
        self.time_step(timestep);
    }

    fn particle_mut(&mut self, x: usize, y: usize) -> &mut Particle {
        let idx = self.get_particle_idx(x, y);
        &mut self.particles[idx]
    }

    fn update_normals(&mut self) {
        for particle in self.particles.iter_mut() {
            particle.reset_normal();
        }

        for x in 0..self.num_particles_width - 1 {
            for y in 0..self.num_particles_height - 1 {
                let normal = Self::calc_triangle_normal(
                    &self.particles[self.get_particle_idx(x + 1, y)],
                    &self.particles[self.get_particle_idx(x, y)],
                    &self.particles[self.get_particle_idx(x, y + 1)],
                );

                self.particle_mut(x + 1, y).add_normal(normal);
                self.particle_mut(x, y).add_normal(normal);
                self.particle_mut(x, y + 1).add_normal(normal);

                let normal = Self::calc_triangle_normal(
                    &self.particles[self.get_particle_idx(x + 1, y + 1)],
                    &self.particles[self.get_particle_idx(x + 1, y)],
                    &self.particles[self.get_particle_idx(x, y + 1)],
                );

                self.particle_mut(x + 1, y + 1).add_normal(normal);
                self.particle_mut(x + 1, y).add_normal(normal);
                self.particle_mut(x, y + 1).add_normal(normal);
            }
        }
    }

    pub fn render<'a, 'b>(
        &'a self,
        camera_bind_group: &'a wgpu::BindGroup,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.diffuse_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.vertex_normal_buffer.slice(..));
        render_pass.set_vertex_buffer(2, self.tex_coord_buffer.slice(..));

        // println!("VERTEX: {:?}", self.particles.len());
        render_pass.draw(0..self.vertices.len() as u32, 0..1);
    }

    pub fn add_force(&mut self, force: Vector3<f32>) {
        for particle in self.particles.iter_mut() {
            particle.add_force(force);
        }
    }

    fn get_particle_idx(&self, x: usize, y: usize) -> usize {
        y * self.num_particles_width + x
    }

    pub fn add_wind_force(&mut self, dir: Vector3<f32>) {
        for x in 0..self.num_particles_width - 1 {
            for y in 0..self.num_particles_height - 1 {
                self.add_wind_forces_for_triangle(
                    self.get_particle_idx(x + 1, y),
                    self.get_particle_idx(x, y),
                    self.get_particle_idx(x, y + 1),
                    dir,
                );
                self.add_wind_forces_for_triangle(
                    self.get_particle_idx(x + 1, y + 1),
                    self.get_particle_idx(x + 1, y),
                    self.get_particle_idx(x, y + 1),
                    dir,
                );
            }
        }
    }

    pub fn time_step(&mut self, timestep: f32) {
        for _ in 0..CONSTRAINT_ITERATIONS {
            for constraint in self.constraints.iter_mut() {
                constraint.satisfy(&mut self.particles);
            }
        }

        for particle in self.particles.iter_mut() {
            particle.time_step(timestep);
        }
    }

    pub fn update_wgpu(&mut self, queue: &wgpu::Queue) {
        Self::fill_vertices(
            &self.particles,
            &mut self.vertices,
            &mut self.normals,
            &mut self.tex_coord,
            self.num_particles_width,
            self.num_particles_height,
        );

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(
            &self.vertex_normal_buffer,
            0,
            bytemuck::cast_slice(&self.normals),
        );
    }
}
