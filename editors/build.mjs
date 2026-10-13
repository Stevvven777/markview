import { build } from "esbuild";
await build({
  entryPoints: ["fonts", "process", "font-store"].map(
    (name) => `vscode/src/shared/${name}.ts`,
  ),
  outdir: "dist",
  bundle: true,
  platform: "node",
  format: "esm",
  packages: "external",
});
