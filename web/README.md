# Markview Web demo

A browser page that renders Markdown onto a `<canvas>` through the Markview
engine compiled to WebAssembly: a textarea feeds Markdown, and `markview-core`
plus `markview-render` do the parsing, layout and painting. The frozen page
contract — JS API, page behavior and acceptance test — lives in
[docs/mvaac-web-demo.md](../docs/mvaac-web-demo.md).

`web/` is a **pnpm monorepo**: [`packages/markview`](packages/markview) is the
reusable `@markview/web` component, `apps/demo` is the thin private demo, and
`web/dist/` is the built, self-contained demo site that `serve.mjs` serves.

The demo consumes the package's **built** `dist/index.js`, not its sources, so
the build below also proves that a downstream bundler gets a working entry
point. The wasm binary travels as a plain sibling file named
`markview_web_bg.wasm`, which `init()` resolves against `import.meta.url`;
nothing depends on a bundler's asset handling. A deployment that serves the
JavaScript away from the binary passes `init({ wasmUrl })`.

## Build

Two steps, in order:

```sh
scripts/build-web.sh      # cargo build (wasm32) + wasm-bindgen -> web/packages/markview/wasm/
pnpm --dir web build      # esbuild + tsc: package dist and the demo site in web/dist/
```

`scripts/build-web.sh` needs `wasm-bindgen` 0.2.129 — vendored under `.tools/`
and picked up through `$WASM_BINDGEN`, or installed with `cargo install
wasm-bindgen-cli --version 0.2.129`. Everything under `web/dist/`,
`web/*/dist/` and `web/packages/markview/wasm/` is generated and gitignored.

## Serve

```sh
node web/serve.mjs    # or: pnpm --dir web serve; PORT selects the port, default 4173
```

Then open `http://127.0.0.1:4173/`. The server is dependency-free, serves the
built site in `web/dist/` with `Cache-Control: no-store`, and gives `.wasm`
files the `application/wasm` MIME type the WebAssembly fetch requires.

## Test

The Playwright acceptance suite (40 checks) drives engine startup, rendering,
selection, copy, incremental re-layout, the resumable layout API, the reader's
lifecycle, reflow on a narrower canvas and the scroll range against the built
demo in `web/dist/`:

```sh
pnpm --dir web test                      # headless, SwiftShader (works anywhere)
MV_GPU=1 pnpm --dir web test --project=chromium-gpu   # the real GPU
```

Headless Chromium picks SwiftShader unless ANGLE is pointed at the platform
driver, so the default project rasterizes on the CPU. The `chromium-gpu`
project adds `--use-angle=vulkan --enable-features=Vulkan` and is opt-in,
because a machine with no Vulkan driver should fail loudly rather than silently
fall back. Each run prints the device it used:

```
[device] wgpu adapter: ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) …), SwiftShader driver) (Gl, Cpu)
[device] wgpu adapter: ANGLE (Intel, Vulkan 1.4.354 (Intel(R) Arc(tm) Graphics (MTL) …), Intel open-source Mesa driver) (Gl, IntegratedGpu)
```

## `@markview/web`

The reusable component. Call `init()` once, then use the surface:

```js
import init, { Markview, CanvasReader } from "@markview/web";
await init();

// Low level: full control of the handle.
const mv = await Markview.create(canvas, { fontSize: 19 });
mv.setMarkdown("# Hello");          // complete layout, published
const update = mv.beginLayout(md);  // resumable: see below
console.log(mv.stats());            // MarkviewStats (blocks, glyphs, pending, ...)

// High level: CanvasReader owns the rAF loop, sizing and input.
const reader = await CanvasReader.attach(canvas, {
  markdown,
  markview: { theme: "dark" },
  stepBudgetMs: 8,
  onStats: (s) => updateHud(s),
});
```

`Markview` is the handle (`setMarkdown`, selection, copy, `stats()`, …);
`CanvasReader` is the convenience that keeps the demo a wiring file. See
`packages/markview/dist/index.d.ts` and the contract for the full surface.

## Progressive layout

Long documents need not be laid out in one go. `beginLayout` starts a pass and
returns a `LayoutUpdate` handle; each `step()` lays out for at most a budget of
milliseconds and publishes the prefix it finished, so a caller can drive it
from `requestAnimationFrame` and render after every step:

```js
const update = mv.beginLayout(longMarkdown);
while (!update.step(8)) {
  mv.frame();            // present the prefix laid out so far
}
```

`step()` never re-lays-out what an earlier call already laid out — the cost of
a step does not grow with the already-published prefix. `finish()` completes
the same pass synchronously, and `update.blocks` / `update.done` report
progress.

## Injecting configuration

Set `window.MV_CONFIG` to an object or a JSON string before the module runs;
the demo hands it to `Markview.create()`, and the engine fills in every key the
config leaves out. Recognised keys: `width`, `fontSize`, `theme`, `justify`,
`hyphenate`, `paragraphIndent`, `greedy`, `hideFrontMatter`,
`frontMatterLabel`:

```js
window.MV_CONFIG = { fontSize: 20, theme: "dark" };
```

## Known limitations

- No tabs, settings UI or font management: the demo is a single document.
- Configuration is injected from JavaScript only — there is no file IO.
- Only the bundled subset faces (embedded by `crates/markview-web/src/fonts.rs`)
  ship with the demo, so exotic scripts may render as tofu. Check any
  document's coverage with `python3 scripts/check_web_font_coverage.py [document]`.
  That check unions every bundled face, so a character only one face carries is
  still reported as covered when the family a body run reaches cannot fall back
  to it: keep an eye on the canvas for a style gap as well.

## Interaction testing

The demo's **Scroll** selector switches between `internal` easing and
`external` direct motion without rebuilding. **Interaction sample** loads
links, a hidden heading in details, a wide code block and a long document for
wheel and selection testing. `internal` is the default; hosts can explicitly choose `external` for
input whose motion is already maintained outside Markview. Link and image callbacks display their targets in the
demo's notice area; the component does not navigate external links by itself.

Hosts can pass `scrollMode`, `onLink` and `onImage` to `CanvasReader.attach`.
Direct integrations can use `scrollInput`, `cursor`, `pointerLeave`,
`cancelPointer` and the activation returned by `pointerUp`; see the contract
for the types and motion ownership semantics.
