import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL(".", import.meta.url));
process.chdir(root);
import { build } from "esbuild";
await build({
  absWorkingDir: root,
  entryPoints: ["fonts", "process", "font-store", "font-client"].map(
    (name) => `vscode/src/shared/${name}.ts`,
  ),
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
