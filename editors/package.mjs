import { spawnSync } from "node:child_process";
import { cp, mkdir, open, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
const root = path.dirname(fileURLToPath(import.meta.url));
process.chdir(root);
const target =
  process.argv[2] ??
  `${process.platform === "win32" ? "win32" : process.platform}-${process.arch}`;
const binary =
  process.env.MARKVIEW_BINARY ??
  path.resolve(
    "../target/release",
    target.startsWith("win32") ? "markview.exe" : "markview",
  );
const handle = await open(binary);
const header = Buffer.alloc(4096);
try {
  await handle.read(header);
} finally {
  await handle.close();
}
let actual;
const cpu = (value, x64, arm64) =>
  value === x64 ? "x64" : value === arm64 ? "arm64" : undefined;
if (header.readUInt32LE(0) === 0xfeedfacf) {
  const arch = cpu(header.readUInt32LE(4), 0x1000007, 0x100000c);
  if (arch) actual = `darwin-${arch}`;
} else if (
  header.subarray(0, 6).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46, 2, 1]))
) {
  const arch = cpu(header.readUInt16LE(18), 62, 183);
  if (arch) actual = `linux-${arch}`;
} else if (header.toString("ascii", 0, 2) === "MZ") {
  const pe = header.readUInt32LE(60);
  if (pe + 6 <= header.length && header.readUInt32LE(pe) === 0x4550) {
    const arch = cpu(header.readUInt16LE(pe + 4), 0x8664, 0xaa64);
    if (arch) actual = `win32-${arch}`;
  }
}
if (actual !== target)
  throw new Error(
    `Exporter is ${actual ?? "unrecognized"}; cannot package it as ${target}`,
  );
await mkdir("vscode/bin", { recursive: true });
await cp(
  binary,
  `vscode/bin/${target.startsWith("win32") ? "markview.exe" : "markview"}`,
);
await mkdir("../artifacts/vscode", { recursive: true });
const folder = "vscode";
const manifest = JSON.parse(await readFile(`${folder}/package.json`, "utf8"));
const output = path.resolve(
  `../artifacts/vscode/${manifest.name}-${manifest.version}-${target}.vsix`,
);
const result = spawnSync(
  process.execPath,
  [
    path.resolve("node_modules/@vscode/vsce/vsce"),
    "package",
    "--no-dependencies",
    "--target",
    target,
    "--out",
    output,
  ],
  { cwd: path.resolve(folder), stdio: "inherit" },
);
if (result.error) throw result.error;
if (result.status) process.exit(result.status);
