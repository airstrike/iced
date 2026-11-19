struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@group(0) @binding(0) var surface: texture_2d<f32>;
@group(0) @binding(1) var surface_sampler: sampler;
@group(0) @binding(2) var<uniform> opacity: f32;

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2., -2.) + vec2<f32>(-1., 1.), 0., 1.), uv);
}
@fragment fn fs_main(vertex: Vertex) -> @location(0) vec4<f32> {
    return textureSample(surface, surface_sampler, vertex.uv) * opacity;
}
