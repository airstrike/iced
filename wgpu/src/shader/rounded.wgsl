// Signed distance to a rounded corner, with each coordinate growing toward
// the outside of its two edges.
fn corner_distance(edges: vec2<f32>, radius: f32) -> f32 {
    let q = edges + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - radius;
}

// p is relative to the frame centre; positive coordinates point right and down.
fn rounded_frame_distance(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    if all(radii <= vec4(min(half_size.x, half_size.y))) {
        let side = select(radii.yz, radii.xw, p.x < 0.0);
        return corner_distance(abs(p) - half_size, select(side.y, side.x, p.y < 0.0));
    }

    // Across a centreline either corner can still reach the pixel. The frame
    // is the intersection of all four rounded corners, including opposite ones.
    let tl = -p - half_size;
    let br = p - half_size;
    return max(
        max(corner_distance(tl, radii.x), corner_distance(vec2(br.x, tl.y), radii.y)),
        max(corner_distance(br, radii.z), corner_distance(vec2(tl.x, br.y), radii.w))
    );
}
