import {
  runTests,
  downloadAndUnzipVSCode,
  resolveCliArgsFromVSCodeExecutablePath,
} from "@vscode/test-electron";
import os from "node:os";
import { spawnSync } from "node:child_process";
import { mkdtemp, readdir, readFile } from "node:fs/promises";
import { build } from "esbuild";
import path from "node:path";
import { fileURLToPath } from "node:url";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
await build({
  entryPoints: [path.join(root, "tests/host.ts")],
  outfile: path.join(root, "dist/host.cjs"),
  bundle: true,
  platform: "node",
  format: "cjs",
  external: ["vscode"],
});
const standalone = process.argv.includes("--export-only"),
  installed = process.argv.includes("--installed");
const executable =
  process.env.VSCODE_EXECUTABLE ??
  (await downloadAndUnzipVSCode({
    extensionDevelopmentPath: path.join(root, "vscode"),
  }));
const profile = await mkdtemp(
  path.join(process.platform === "win32" ? os.tmpdir() : "/tmp", "mv-code-"),
);
const profileArgs = [
  "--user-data-dir",
  path.join(profile, "user"),
  "--extensions-dir",
  path.join(profile, "extensions"),
];
const folders = ["vscode"];
let developmentPaths = folders.map((folder) => path.join(root, folder));
if (installed) {
  const [cli, ...cliArgs] = resolveCliArgsFromVSCodeExecutablePath(executable, {
    reuseMachineInstall: true,
  });
  for (const folder of folders) {
    const manifest = JSON.parse(
      await readFile(path.join(root, folder, "package.json"), "utf8"),
    );
    const target = `${process.platform}-${process.arch}`;
    const vsix = path.resolve(
      root,
      "../artifacts/vscode",
      `${manifest.name}-${manifest.version}-${target}.vsix`,
    );
    const result = spawnSync(
      cli,
      [...cliArgs, ...profileArgs, "--install-extension", vsix, "--force"],
      { stdio: "inherit", shell: process.platform === "win32" },
    );
    if (result.error) throw result.error;
    if (result.status) throw new Error(`VSIX installation failed: ${folder}`);
  }
  const directory = path.join(profile, "extensions"),
    entries = await readdir(directory);
  developmentPaths = folders.map((folder) => {
    const entry = entries.find((file) => file.startsWith("szdytom.markview-"));
    if (!entry) throw new Error(`Installed extension is missing: ${folder}`);
    return path.join(directory, entry);
  });
}
await runTests({
  vscodeExecutablePath: executable,
  extensionDevelopmentPath: developmentPaths,
  extensionTestsPath: path.join(root, "dist/host.cjs"),
  extensionTestsEnv: {
    MARKVIEW_TEST_ROOT: root,
    MARKVIEW_TEST_PREVIEW: standalone ? "0" : "1",
    MARKVIEW_TEST_INSTALLED: installed ? "1" : "0",
  },
  launchArgs: [
    "--disable-extensions",
    "--skip-welcome",
    "--skip-release-notes",
    "--disable-workspace-trust",
    "--disable-renderer-backgrounding",
    "--disable-backgrounding-occluded-windows",
    ...(process.env.CI
      ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
      : []),
    ...profileArgs,
  ],
});
