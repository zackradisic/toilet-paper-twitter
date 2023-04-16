struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
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
fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var ret: vec4<f32> = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    let world_normal = normalize(vec3<f32>(50.0, 6.0, 50.0));
    // let world_normal = normalize(vec3<f32>(camera.view_pos.xyz));
    let min = 0.0;
    let diffuse_strength = max(dot(in.normal, world_normal), min);
    var color: vec3<f32> = vec3<f32>(ret.xyz) * diffuse_strength;
    // if (in.in_vertex_index <= 2u) {
    //     color = vec3<f32>(1.0, 0.0, 0.0);
    // }
    return vec4<f32>(color, 1.0);
}
