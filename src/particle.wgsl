struct Camera {
    view_proj: mat4x4<f32>,
    dimensions: vec2<f32>,
    scale: f32,
};

@binding(0) @group(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) in_vertex_index: u32,
}

@vertex
fn vs_main(in: VertexInput, @builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let pos = camera.view_proj * vec4<f32>(in.pos, 1.0);

    out.normal = in.normal;
    out.position = pos;
    out.tex_coords = in.tex_coord;
    out.in_vertex_index = in_vertex_index;

    return out;
}

@fragment @group(1) @binding(0)
var t_diffuse: texture_2d<f32>;

@fragment @group(1) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let ret = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let world_normal = normalize(vec3<f32>(50.0, 6.0, 50.0));
    let diffuse_strength = max(dot(in.normal, world_normal), 0.8);
    return ret * diffuse_strength;
}

// @fragment
// fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
//     let ret = textureSample(t_diffuse, s_diffuse, in.tex_coords);
//     let camera_pos = vec3<f32>(0.0, 0.0, 30.0);
//     let look_pos = vec3<f32>(5.0, 0.0, 0.0);
//     let world_dir = look_pos - camera_pos;
//     // let world_normal = normalize(vec3<f32>(50.0, 6.0, 50.0));
//     let world_normal = normalize(world_dir);
//     let diffuse_strength = max(dot(in.normal, world_normal), 0.0) * 2.0;
//     return (ret * 0.1) * diffuse_strength;
// }
