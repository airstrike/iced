// Colors enter as straight, linear-light RGBA in both storage paths.
override GRADIENT_WEB_COLORS: bool = false;
@group(1) @binding(0) var gradient_table: texture_2d<f32>;

struct GradientPage {
    count: u32,
    has_previous: u32,
    has_next: u32,
    padding: u32,
}
@group(1) @binding(1) var<uniform> gradient_page: GradientPage;

struct GradientStop {
    color: vec4<f32>,
    offset: f32,
}

fn gradient_texel(index: u32) -> vec4<f32> {
    let width = textureDimensions(gradient_table).x;
    return textureLoad(gradient_table, vec2<i32>(i32(index % width), i32(index / width)), 0);
}

fn table_stop(index: u32) -> GradientStop {
    let texel = index * 2u;
    return GradientStop(gradient_texel(texel), gradient_texel(texel + 1u).x);
}

fn gradient_offset(point: vec2<f32>, direction: vec4<f32>, kind: u32) -> f32 {
    let delta = point - direction.xy;
    if kind == 1u {
        return length(delta) / max(direction.z, 1e-10);
    }
    if kind == 2u {
        let angle = atan2(delta.y, delta.x) + 1.570796327 - direction.z;
        return fract(angle / 6.283185307);
    }
    let axis = direction.zw - direction.xy;
    let length_squared = dot(axis, axis);
    if length_squared == 0.0 {
        return 0.5;
    }
    return dot(delta, axis) / length_squared;
}

fn gradient_segment(t: f32, left: GradientStop, right: GradientStop) -> vec4<f32> {
    if t < left.offset {
        return left.color;
    }
    if t >= right.offset {
        return right.color;
    }
    return mix(left.color, right.color, (t - left.offset) / (right.offset - left.offset));
}

fn gradient_sample(
    t: f32,
    first: vec4<f32>,
    second: vec4<f32>,
    offsets: vec2<f32>,
    base: u32,
    count: u32,
) -> vec4<f32> {
    if count == 0u {
        return vec4<f32>(0.0);
    }
    if count == 1u {
        return first;
    }
    if count == 2u {
        return gradient_segment(t, GradientStop(first, offsets.x), GradientStop(second, offsets.y));
    }

    // Adjacent texture pages share a stop. Only one page owns each point, including
    // geometry outside the padded ends of the ramp (shadows and borders included).
    let end = gradient_page.count;
    if gradient_page.has_previous != 0u && t < gradient_texel(base * 2u + 1u).x {
        discard;
    }
    if gradient_page.has_next != 0u && t >= gradient_texel((base + end - 1u) * 2u + 1u).x {
        discard;
    }
    // Upper bound makes the last authored stop at an equal offset own the boundary.
    var lo = 0u;
    var hi = end;
    loop {
        if lo >= hi { break; }
        let mid = lo + (hi - lo) / 2u;
        if gradient_texel((base + mid) * 2u + 1u).x <= t {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    if lo == 0u {
        return gradient_texel(base * 2u);
    }
    if lo == end {
        return gradient_texel((base + end - 1u) * 2u);
    }
    return gradient_segment(t, table_stop(base + lo - 1u), table_stop(base + lo));
}

fn gradient_color(
    point: vec2<f32>,
    direction: vec4<f32>,
    descriptor: vec3<u32>,
    first: vec4<f32>,
    second: vec4<f32>,
    offsets: vec2<f32>,
) -> vec4<f32> {
    let t = gradient_offset(point, direction, descriptor.x);
    let color = gradient_sample(t, first, second, offsets, descriptor.y, descriptor.z);
    var rgb = color.rgb;
    if GRADIENT_WEB_COLORS {
        rgb = select(1.055 * pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055,
                     rgb * 12.92, rgb <= vec3<f32>(0.0031308));
    }
    return vec4<f32>(rgb * color.a, color.a);
}
