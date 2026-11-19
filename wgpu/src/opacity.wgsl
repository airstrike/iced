struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@group(0) @binding(0) var surface: texture_2d<f32>;
@group(0) @binding(1) var surface_sampler: sampler;
struct Mask {
    metadata: vec4<f32>,
    frame: vec4<f32>,
    radii: vec4<f32>,
}
@group(0) @binding(2) var<uniform> mask: Mask;

fn coverage(uv: vec2<f32>) -> f32 {
    let p = uv * vec2<f32>(textureDimensions(surface)) - mask.frame.xy;
    let half_size = mask.frame.zw * 0.5;
    let left = p.x < half_size.x;
    let top = p.y < half_size.y;
    let r = select(select(mask.radii.z, mask.radii.w, left), select(mask.radii.y, mask.radii.x, left), top);
    if r <= 0. { return mask.metadata.x; }
    let d = abs(p - half_size) - half_size + vec2<f32>(r);
    if d.x <= 0. || d.y <= 0. { return mask.metadata.x; }
    return mask.metadata.x * clamp(r + 0.5 - length(max(d, vec2<f32>(0.))), 0., 1.);
}

@group(0) @binding(3) var black: texture_2d<f32>;

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2., -2.) + vec2<f32>(-1., 1.), 0., 1.), uv);
}
@fragment fn fs_main(vertex: Vertex) -> @location(0) vec4<f32> {
    return textureSample(surface, surface_sampler, vertex.uv) * coverage(vertex.uv);
}

// With Normal and Multiply only, the opaque-white response is the
// transparent response plus the uncovered fraction (1 - source alpha).
// H = T.rgb - B. The group maps premultiplied (d, a) to
// B + (1 - T.a + H) * d + H * (1 - a).
@fragment fn fs_pass_scale(vertex: Vertex) -> @location(0) vec4<f32> {
    let t = textureSample(surface, surface_sampler, vertex.uv);
    let b = textureSample(black, surface_sampler, vertex.uv).rgb;
    return vec4<f32>(vec3<f32>(1.) + coverage(vertex.uv) * (t.rgb - b - vec3<f32>(t.a)), 1.);
}
@fragment fn fs_pass_black(vertex: Vertex) -> @location(0) vec4<f32> {
    return textureSample(black, surface_sampler, vertex.uv) * coverage(vertex.uv);
}
@fragment fn fs_pass_uncovered(vertex: Vertex) -> @location(0) vec4<f32> {
    let transparent_value = textureSample(surface, surface_sampler, vertex.uv);
    let black_value = textureSample(black, surface_sampler, vertex.uv).rgb;
    return vec4<f32>(transparent_value.rgb - black_value, transparent_value.a) * coverage(vertex.uv);
}
