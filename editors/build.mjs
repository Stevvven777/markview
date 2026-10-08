import { build } from "esbuild";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { cp, mkdir, access, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
const root = path.dirname(fileURLToPath(import.meta.url));
process.chdir(root);
const folder = "vscode";
await mkdir(`${folder}/dist`, { recursive: true });
await build({
  absWorkingDir: root,
  entryPoints: [`${folder}/src/extension.ts`],
  outfile: `${folder}/dist/extension.cjs`,
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  external: ["vscode"],
  sourcemap: true,
});
await cp("../assets/icons/markview-256.png", `${folder}/icon.png`);
await cp("../LICENSE", `${folder}/LICENSE`);
await rm("vscode/media", { recursive: true, force: true });
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
await cp("../assets/markview-icon-color.svg", "vscode/media/brand.svg");
await mkdir("dist", { recursive: true });
await build({
  absWorkingDir: root,
  entryPoints: {
    fonts: "vscode/src/shared/fonts.ts",
    "preview-fonts": "vscode/src/shared/preview-fonts.ts",
    process: "vscode/src/shared/process.ts",
    "download-fixture": "tests/download-fixture.ts",
  },
  outdir: "dist",
  bundle: true,
  packages: "external",
  platform: "node",
  format: "esm",
  target: "node20",
});
await mkdir("vscode/resources", { recursive: true });
const binary =
  process.env.MARKVIEW_BINARY ??
  path.resolve(
    "../target/release",
    process.platform === "win32" ? "markview.exe" : "markview",
  );
const { stdout } = await promisify(execFile)(binary, [
  "export",
  "--catalog",
  "--destination",
  "ui",
]);
await writeFile(
  "vscode/resources/ui-fonts.json",
  JSON.stringify(JSON.parse(stdout)),
);
console.log("Built extension and bundled viewer assets.");
