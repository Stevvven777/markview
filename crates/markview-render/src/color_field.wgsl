struct FieldVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) metrics: vec3<f32>,
};

@vertex fn vs(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) metrics: vec4<f32>) -> FieldVertex {
    var out: FieldVertex;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.metrics = metrics.xyz;
    return out;
}

// `rgba` converts the existing sRGB `0xRRGGBBAA` palette to linear RGBA.
fn rgba(value: u32) -> vec4<f32> {
    let c = vec4<f32>(f32((value >> 24u) & 255u), f32((value >> 16u) & 255u), f32((value >> 8u) & 255u), f32(value & 255u)) / 255.0;
    let rgb = select(pow((c.rgb + 0.055) / 1.055, vec3<f32>(2.4)), c.rgb / 12.92, c.rgb <= vec3<f32>(0.04045));
    return vec4<f32>(rgb, c.a);
}

fn field_over(foreground: vec4<f32>, background: vec4<f32>) -> vec4<f32> {
    let remaining = background.a * (1.0 - foreground.a);
    let alpha = foreground.a + remaining;
    if alpha == 0.0 { return vec4<f32>(0.0); }
    return vec4<f32>((foreground.rgb * foreground.a + background.rgb * remaining) / alpha, alpha);
}

@fragment fn field_fs(in: FieldVertex) -> @location(0) vec4<f32> {
    return clamp(field_color(in.uv.x, in.uv.y, in.metrics.xy, in.metrics.z), vec4<f32>(0.0), vec4<f32>(1.0));
}
