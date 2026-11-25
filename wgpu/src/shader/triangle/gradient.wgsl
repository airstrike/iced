struct GradientVertexInput {
    @location(0) v_pos: vec2<f32>,
    @location(1) first: vec4<f32>,
    @location(2) second: vec4<f32>,
    @location(3) offsets: vec2<f32>,
    @location(4) direction: vec4<f32>,
    @location(5) descriptor: vec3<u32>,
}

struct GradientVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) raw_position: vec2<f32>,
    @location(1) @interpolate(flat) first: vec4<f32>,
    @location(2) @interpolate(flat) second: vec4<f32>,
    @location(3) @interpolate(flat) offsets: vec2<f32>,
    @location(4) @interpolate(flat) direction: vec4<f32>,
    @location(5) @interpolate(flat) descriptor: vec3<u32>,
}

@vertex
fn gradient_vs_main(input: GradientVertexInput) -> GradientVertexOutput {
    var out: GradientVertexOutput;
    out.position = globals.transform * vec4<f32>(input.v_pos, 0.0, 1.0);
    out.raw_position = input.v_pos;
    out.first = input.first;
    out.second = input.second;
    out.offsets = input.offsets;
    out.direction = input.direction;
    out.descriptor = input.descriptor;
    return out;
}

@fragment
fn gradient_fs_main(input: GradientVertexOutput) -> @location(0) vec4<f32> {
    return gradient_color(input.raw_position, input.direction, input.descriptor,
        input.first, input.second, input.offsets);
}
