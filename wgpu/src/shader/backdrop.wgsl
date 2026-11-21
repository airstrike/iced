// Blend modes a fixed-function blend cannot express. The compositor copies the
// target under the surface into `backdrop` and replaces those pixels with the
// W3C Compositing and Blending Level 1 result:
//
//   co = cs * (1 - ab) + cb * (1 - as) + as * ab * B(Cb, Cs)
//   ao = as + ab * (1 - as)
//
// where cs and cb are premultiplied and Cs and Cb are not. Scaling the
// premultiplied source by its coverage mixes the backdrop with this result,
// so opacity and clip masks need no separate pass.
@group(1) @binding(0) var backdrop: texture_2d<f32>;

const DARKEN: u32 = 1u;
const COLOR_BURN: u32 = 2u;
const LIGHTEN: u32 = 3u;
const COLOR_DODGE: u32 = 4u;
const OVERLAY: u32 = 5u;
const SOFT_LIGHT: u32 = 6u;
const HARD_LIGHT: u32 = 7u;
const DIFFERENCE: u32 = 8u;
const EXCLUSION: u32 = 9u;

// Unpremultiplied colors read back from 8-bit surfaces are rarely exactly 0
// or 1. The dodge and burn special cases compare with this tolerance.
const EPSILON: f32 = 1e-4;

fn unpremultiply(color: vec4<f32>) -> vec3<f32> {
    if color.a <= 0. { return vec3<f32>(0.); }
    return clamp(color.rgb / color.a, vec3<f32>(0.), vec3<f32>(1.));
}

fn screen(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    return cb + cs - cb * cs;
}

fn hard_light(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    return select(screen(cb, 2. * cs - 1.), cb * 2. * cs, cs <= vec3<f32>(0.5));
}

fn color_dodge(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let dodge = min(vec3<f32>(1.), cb / max(vec3<f32>(1.) - cs, vec3<f32>(EPSILON)));
    let saturated = select(dodge, vec3<f32>(1.), cs >= vec3<f32>(1. - EPSILON));
    return select(saturated, vec3<f32>(0.), cb <= vec3<f32>(EPSILON));
}

fn color_burn(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let burn = vec3<f32>(1.) - min(vec3<f32>(1.), (vec3<f32>(1.) - cb) / max(cs, vec3<f32>(EPSILON)));
    let saturated = select(burn, vec3<f32>(0.), cs <= vec3<f32>(EPSILON));
    return select(saturated, vec3<f32>(1.), cb >= vec3<f32>(1. - EPSILON));
}

fn soft_light(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(cb), ((16. * cb - 12.) * cb + 4.) * cb, cb <= vec3<f32>(0.25));
    let darker = cb - (1. - 2. * cs) * cb * (1. - cb);
    let lighter = cb + (2. * cs - 1.) * (d - cb);
    return select(lighter, darker, cs <= vec3<f32>(0.5));
}

fn separable(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    switch mode {
        case DARKEN: { return min(cb, cs); }
        case COLOR_BURN: { return color_burn(cb, cs); }
        case LIGHTEN: { return max(cb, cs); }
        case COLOR_DODGE: { return color_dodge(cb, cs); }
        case OVERLAY: { return hard_light(cs, cb); }
        case SOFT_LIGHT: { return soft_light(cb, cs); }
        case HARD_LIGHT: { return hard_light(cb, cs); }
        case DIFFERENCE: { return abs(cb - cs); }
        case EXCLUSION: { return cb + cs - 2. * cb * cs; }
        default: { return cs; }
    }
}

@fragment fn fs_backdrop(vertex: Vertex) -> @location(0) vec4<f32> {
    let sample = textureSample(surface, surface_sampler, vertex.uv);
    let source = sample * coverage(vertex.uv);
    let texel = vec2<i32>(floor(vertex.position.xy - mask.backdrop.xy));
    let destination = textureLoad(backdrop, texel, 0);
    let blended = separable(u32(mask.metadata.y), unpremultiply(destination), unpremultiply(sample));
    let color = source.rgb * (1. - destination.a)
        + destination.rgb * (1. - source.a)
        + source.a * destination.a * blended;
    return vec4<f32>(color, source.a + destination.a * (1. - source.a));
}
