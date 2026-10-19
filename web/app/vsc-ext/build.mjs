import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL(".", import.meta.url));
process.chdir(root);
import path from "node:path";
import { build } from "esbuild";
await build({
  absWorkingDir: root,
  entryPoints: [
    "fonts",
    "process",
    "font-store",
    "font-client",
    "export-snapshot",
  ].map((name) => `vscode/src/shared/${name}.ts`),
  outdir: "dist",
  bundle: true,
  platform: "node",
  format: "esm",
  packages: "external",
});

import { cp, mkdir } from "node:fs/promises";
await build({
  absWorkingDir: root,
  entryPoints: ["vscode/src/extension.ts"],
  outfile: "vscode/dist/extension.cjs",
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  external: ["vscode"],
});
await mkdir("vscode/media", { recursive: true });
await build({
  absWorkingDir: root,
  entryPoints: ["vscode/src/shared/font-panel-webview.ts"],
  outfile: "vscode/media/fonts.js",
  bundle: true,
  platform: "browser",
  target: "es2022",
});
await cp("vscode/src/shared/font-panel.css", "vscode/media/fonts.css");
await cp("../../../assets/markview-icon-color.svg", "vscode/media/brand.svg");
await cp("../../../assets/icons/markview-256.png", "vscode/icon.png");
await cp("../../../LICENSE", "vscode/LICENSE");

await build({
  absWorkingDir: root,
  entryPoints: ["vscode/src/preview/webview.ts"],
  outfile: "vscode/media/preview.js",
  bundle: true,
  format: "esm",
  platform: "browser",
  target: "es2022",
  alias: {
    "@markview/viewer": path.resolve("../../packages/markview/src/index.ts"),
    "@markview/fonts": path.resolve("../../packages/fonts/src/index.ts"),
    "@markview/resources": path.resolve(
      "../../packages/resources/src/index.ts",
    ),
  },
});
await cp(
  "../../packages/markview/wasm/markview_web_bg.wasm",
  "vscode/media/markview_web_bg.wasm",
);
await cp("vscode/src/preview/preview.css", "vscode/media/preview.css");
for (const name of ["preview-light", "preview-dark"])
  await cp(`../../../assets/ui/${name}.svg`, `vscode/media/${name}.svg`);
