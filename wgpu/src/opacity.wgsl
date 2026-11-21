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
    source_matrix: vec4<f32>,
    clip_rotation: vec4<f32>,
    // The target pixel copied to texel (0, 0) of the backdrop.
    backdrop: vec4<f32>,
}
@group(0) @binding(2) var<uniform> mask: Mask;

fn coverage(uv: vec2<f32>) -> f32 {
    if any(uv < vec2<f32>(0.)) || any(uv > vec2<f32>(1.)) { return 0.; }
    if mask.clip_rotation.z == 0. { return mask.metadata.x; }
    let half_size = mask.frame.zw * 0.5;
    let offset = uv * vec2<f32>(textureDimensions(surface)) - mask.frame.xy - half_size;
    let c = mask.clip_rotation.x;
    let s = mask.clip_rotation.y;
    let p = vec2<f32>(c*offset.x - s*offset.y, s*offset.x + c*offset.y);
    let left = p.x < 0.;
    let top = p.y < 0.;
    let r = select(select(mask.radii.z, mask.radii.w, left), select(mask.radii.y, mask.radii.x, left), top);
    let d = abs(p) - half_size + vec2<f32>(r);
    let distance = length(max(d, vec2<f32>(0.))) + min(max(d.x,d.y),0.) - r;
    return mask.metadata.x * clamp(0.5-distance,0.,1.);
}

@group(0) @binding(3) var black: texture_2d<f32>;
@group(0) @binding(4) var white: texture_2d<f32>;

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    let local = uv - vec2<f32>(0.5);
    let source_uv = vec2<f32>(dot(mask.source_matrix.xy,local),dot(mask.source_matrix.zw,local))+vec2<f32>(0.5);
    return Vertex(vec4<f32>(uv * vec2<f32>(2., -2.) + vec2<f32>(-1., 1.), 0., 1.), source_uv);
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
// Screen breaks that relation, so the group is also evaluated over opaque
// white W. Any affine response is T + (W - B) * d - H * a, and only the
// backdrop scale differs from the two-surface case.
@fragment fn fs_pass_scale_white(vertex: Vertex) -> @location(0) vec4<f32> {
    let b = textureSample(black, surface_sampler, vertex.uv).rgb;
    let w = textureSample(white, surface_sampler, vertex.uv).rgb;
    return vec4<f32>(vec3<f32>(1.) + coverage(vertex.uv) * (w - b - vec3<f32>(1.)), 1.);
}
@fragment fn fs_pass_black(vertex: Vertex) -> @location(0) vec4<f32> {
    return textureSample(black, surface_sampler, vertex.uv) * coverage(vertex.uv);
}
@fragment fn fs_pass_uncovered(vertex: Vertex) -> @location(0) vec4<f32> {
    let transparent_value = textureSample(surface, surface_sampler, vertex.uv);
    let black_value = textureSample(black, surface_sampler, vertex.uv).rgb;
    return vec4<f32>(transparent_value.rgb - black_value, transparent_value.a) * coverage(vertex.uv);
}
