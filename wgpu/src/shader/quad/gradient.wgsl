struct GradientVertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) first: vec4<f32>,
    @location(1) second: vec4<f32>,
    @location(2) offsets: vec2<f32>,
    @location(3) direction: vec4<f32>,
    @location(4) descriptor: vec3<u32>,
    @location(5) position_and_scale: vec4<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) border_radius: vec4<f32>,
    @location(8) border_width: f32,
    @location(9) shadow_color: vec4<f32>,
    @location(10) shadow_offset: vec2<f32>,
    @location(11) shadow_blur_radius: f32,
    @location(12) snap: u32,
}

struct GradientVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) first: vec4<f32>,
    @location(1) @interpolate(flat) second: vec4<f32>,
    @location(2) @interpolate(flat) offsets: vec2<f32>,
    @location(3) @interpolate(flat) direction: vec4<f32>,
    @location(4) @interpolate(flat) descriptor: vec3<u32>,
    @location(5) pos: vec2<f32>,
    @location(6) scale: vec2<f32>,
    @location(7) border_color: vec4<f32>,
    @location(8) border_radius: vec4<f32>,
    @location(9) border_width: f32,
    @location(10) shadow_color: vec4<f32>,
    @location(11) shadow_offset: vec2<f32>,
    @location(12) shadow_blur_radius: f32,
}

@vertex
fn gradient_vs_main(input: GradientVertexInput) -> GradientVertexOutput {
    var out: GradientVertexOutput;

    let in_pos = input.position_and_scale.xy;
    let in_scale = input.position_and_scale.zw;

    var bounds = shadow_expanded_bounds(in_pos, in_scale, input.shadow_offset, input.shadow_blur_radius) * globals.scale;
    var pos: vec2<f32> = bounds.xy;
    var scale: vec2<f32> = bounds.zw;

    var pos_snap = vec2<f32>(0.0, 0.0);
    var scale_snap = vec2<f32>(0.0, 0.0);

    if bool(input.snap) {
        pos_snap = round(pos + nudge) - pos;
        scale_snap = round(pos + scale + nudge) - pos - pos_snap - scale;
    }

    var min_border_radius = min(in_scale.x, in_scale.y) * 0.5;
    var border_radius: vec4<f32> = vec4<f32>(
        min(input.border_radius.x, min_border_radius),
        min(input.border_radius.y, min_border_radius),
        min(input.border_radius.z, min_border_radius),
        min(input.border_radius.w, min_border_radius)
    );

    var transform: mat4x4<f32> = mat4x4<f32>(
        vec4<f32>(scale.x + scale_snap.x + 1.0, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, scale.y + scale_snap.y + 1.0, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(pos + pos_snap - vec2<f32>(0.5, 0.5), 0.0, 1.0)
    );

    out.position = globals.transform * transform * vec4<f32>(vertex_position(input.vertex_index), 0.0, 1.0);
    out.first = input.first;
    out.second = input.second;
    out.offsets = input.offsets;
    out.direction = input.direction;
    out.descriptor = input.descriptor;
    out.pos = in_pos * globals.scale + pos_snap;
    out.scale = in_scale * globals.scale + scale_snap;
    out.border_color = premultiply(input.border_color);
    out.border_radius = border_radius * globals.scale;
    out.border_width = input.border_width * globals.scale;
    out.shadow_color = premultiply(input.shadow_color);
    out.shadow_offset = input.shadow_offset * globals.scale;
    out.shadow_blur_radius = input.shadow_blur_radius * globals.scale;

    return out;
}

@fragment
fn gradient_fs_main(input: GradientVertexOutput) -> @location(0) vec4<f32> {
    // The conic angle is dimensionless. Only the sample position is converted to logical pixels.
    var mixed_color = gradient_color(input.position.xy / globals.scale, input.direction,
        input.descriptor, input.first, input.second, input.offsets);

    var dist: f32 = rounded_box_sdf(
        -(input.position.xy - input.pos - input.scale / 2.0) * 2.0,
        input.scale,
        input.border_radius * 2.0
    ) / 2.0;

    if (input.border_width > 0.0) {
        mixed_color = mix(
            mixed_color,
            input.border_color,
            clamp(0.5 + dist + input.border_width, 0.0, 1.0)
        );
    }

    var quad_alpha: f32 = clamp(0.5 - dist, 0.0, 1.0);
    let quad_color = mixed_color * quad_alpha;

    if input.shadow_color.a > 0.0 {
        return mix_shadow(
            quad_color,
            quad_alpha,
            input.position.xy,
            input.pos,
            input.scale,
            input.border_radius,
            input.shadow_color,
            input.shadow_offset,
            input.shadow_blur_radius
        );
    }

    return quad_color;
}
