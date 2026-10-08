# Color fields

`markview_core::scene::ColorField` paints a component directly with a wgpu
fragment shader. The Rust builder accepts **WGSL function bodies**. Each body
receives normalized `x` and `y` coordinates from zero to one across the original
rectangle, its logical `size: vec2<f32>`, and device `scale: f32`. Clipping keeps
the original coordinates; movement, resizing and DPI changes reuse the program.

```rust
use markview_core::scene::{ColorField, Rect};

let field = ColorField::builder()
    .paint("return rgba(0xEFE0CDFFu);")
    .paint(r#"
        let p = vec2<f32>(x - 0.1, y - 0.5);
        let light = exp(-40.0 * dot(p, p));
        return vec4<f32>(rgba(0xC37C54FFu).rgb, light * 0.2);
    "#)
    .map(r#"
        return vec4<f32>(color.rgb, color.a * smoothstep(0.0, 0.05, x));
    "#)
    .finish();
let draw = field.draw(Rect { x: 24.0, y: 16.0, w: 180.0, h: 32.0 });
```

The canvas starts transparent. Call `paint` repeatedly to composite later layers
over earlier ones using source-over in linear light. Each body returns
`vec4<f32>` with **linear RGB and straight alpha**, in the range zero to one.
The provided `rgba(0xRRGGBBAAu)` helper converts an sRGB palette color to this
representation. `map` also receives the accumulated `color: vec4<f32>` and
replaces it, allowing masks or color transforms. Layers retain floating-point
precision until the final render target conversion.

All bodies form one fragment program, evaluated at device-pixel centers on every
repaint. One field uses one quad and one draw call, with no CPU pixel buffer,
intermediate texture or image upload. There is no raster resolution to choose.
The GPU still performs the field's calculations for each visible fragment; keep
expensive loops and layer counts appropriate for the component's size.

Keep the field or its draws alive to reuse the compiled pipeline. Pipelines are
created on first visible use and cached by shader source in each renderer.
Clones share source storage; independently built identical programs also share
one pipeline. Programs belonging to live components remain cached while
outside the viewport, and unused programs are retired after their owners and
current-frame draws are gone. Changing a function or palette creates a new
field and pipeline; changing only the rectangle or DPI does not.
`Renderer::color_field_stats()` reports cached pipelines and total compilations.
`gpu_bytes()` tracks textures and geometry buffers, excluding driver-managed
pipeline memory.

Bodies are trusted application code, validated by wgpu when compiled. Use WGSL
syntax and valid color values, and reserve the `field_` prefix for generated
functions. This API does not execute Rust closures or accept expressions from
Markdown or MVSS. Color fields are UI-only and omitted from PDF export, like
reader icons.

## Native Amber tab example

This example uses a Buttercream base, faint Frosted Almond grain, a Caramel glow,
and a Sweet Cream light spot. Four `paint` calls build the material; a final
`map` applies an antialiased rounded mask. The label, close icon, border and two
fine lines use ordinary native draws.

![Native GPU Amber tab at 3× scale](../screenshots/amber-tab-color-field.png)

The image is actual Metal renderer output at 3× scale: an isolated 180 × 32
logical-pixel tab with a 178 × 30 color-field interior. See the complete
[`examples/amber-tab.rs`](../../examples/amber-tab.rs) for the WGSL layers and
rounded-distance mask. The example uses MVSS only for its panel palette.

```sh
cargo run --release --example amber-tab
```

This writes 1×, 1.25× and 3× PNGs into `artifacts/amber-tabs`, repaints each
result 100 times and checks that tracked GPU memory and the pipeline compilation
count stay constant across repaints and scale changes. To reproduce the
committed screenshot, copy `artifacts/amber-tabs/amber-field-3x.png` to
`docs/screenshots/amber-tab-color-field.png`. Text uses host fonts, so glyphs
can vary between machines.
