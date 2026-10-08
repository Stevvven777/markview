// Local diagnostic: cold browser contexts, real system fonts, production MVaaC.
import { build } from "esbuild";
import { chromium } from "../../web/node_modules/@playwright/test/index.mjs";
import { createServer } from "node:http";
import { readFile, stat, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { FontIndex } from "../dist/fonts.js";
const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const storage = process.argv[2];
if (!storage) throw new Error("Pass the Markview global-storage directory");
const index = new FontIndex(
  path.join(storage, "fonts"),
  path.join(storage, "font-index.json"),
);
const scanStart = performance.now();
await index.refresh();
const scanMs = performance.now() - scanStart;
const catalog = JSON.parse(
  await readFile(path.join(root, "editors/vscode/resources/ui-fonts.json")),
);
const full = index.paths(catalog);
const plain = index.paths({
  ...catalog,
  definitions: catalog.definitions.filter((d) => d.id !== "emoji"),
});
const body = index.paths({
  ...catalog,
  definitions: catalog.definitions.filter((d) =>
    ["serif", "serif[cjk]", "monospace"].includes(d.id),
  ),
});
const markdown = process.argv[3]
  ? await readFile(process.argv[3], "utf8")
  : "# Preview performance fixture\n\n" +
    "Reading follows source positions in both directions. 中文排版。\n\n".repeat(
      90,
    );
const bundle = await build({
  stdin: {
    contents: `export {init, FontSet, Viewer} from './web/packages/markview/src/index.ts';`,
    resolveDir: root,
  },
  bundle: true,
  write: false,
  format: "esm",
  platform: "browser",
});
const server = createServer(async (req, res) => {
  try {
    if (req.url === "/") {
      res.setHeader("Content-Type", "text/html");
      res.end('<main style="width:640px;height:800px"></main>');
      return;
    }
    if (req.url === "/module.js") {
      res.setHeader("Content-Type", "text/javascript");
      res.end(bundle.outputFiles[0].contents);
      return;
    }
    const file =
      req.url === "/viewer.wasm"
        ? path.join(root, "web/packages/markview/wasm/markview_web_bg.wasm")
        : full[Number(req.url.slice(6))];
    if (!file) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.setHeader(
      "Content-Type",
      req.url.endsWith("wasm")
        ? "application/wasm"
        : "application/octet-stream",
    );
    res.end(await readFile(file));
  } catch (error) {
    res.writeHead(500);
    res.end(String(error));
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const browser = await chromium.launch({
  args: ["--enable-unsafe-swiftshader"],
});
const results = [];
try {
  for (const [name, files] of [
    ["full", full],
    ["without-emoji", plain],
    ["body-fonts-only-experiment", body],
  ]) {
    for (let run = 0; run < 3; run++) {
      const context = await browser.newContext();
      const page = await context.newPage();
      await page.goto(`http://127.0.0.1:${server.address().port}/`);
      const result = await page.evaluate(
        async ({ urls, markdown, profileUpdates }) => {
          const { init, FontSet, Viewer } = await import("/module.js");
          const start = performance.now();
          const faces = await Promise.all(
            urls.map(
              async (url) =>
                new Uint8Array(await (await fetch(url)).arrayBuffer()),
            ),
          );
          const fetched = performance.now();
          await init({ wasmUrl: "/viewer.wasm" });
          const initialized = performance.now();
          const fonts = await FontSet.create(faces);
          const decoded = performance.now();
          let done;
          const rendered = new Promise((resolve) => (done = resolve));
          const viewer = await Viewer.mount(document.querySelector("main"), {
            markdown,
            fonts,
            markview: { fontSize: 18, width: 760 },
            onStats: (stats) => {
              if (!stats.pending) done();
            },
          });
          await rendered;
          await new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          );
          const finish = performance.now();
          const edits = [];
          if (profileUpdates) {
            for (const [label, source] of [
              ["trial", markdown],
              ["long", markdown.repeat(10)],
              [
                "single-paragraph",
                "Mixed 中文 text for line breaking. ".repeat(1000),
              ],
            ]) {
              for (let edit = 0; edit < 4; edit++) {
                const completion = new Promise((resolve) => (done = resolve));
                const began = performance.now();
                const middle = Math.floor(source.length / 2);
                viewer.setMarkdown(
                  source.slice(0, middle) +
                    " updated " +
                    edit +
                    source.slice(middle),
                );
                const dispatched = performance.now();
                await completion;
                const stats = viewer.reader.markview.stats();
                edits.push({
                  label,
                  edit,
                  chars: source.length,
                  synchronousMs: dispatched - began,
                  completeFrameMs: performance.now() - began,
                  parseMs: stats.parseMs,
                  layoutMs: stats.layoutMs,
                  reused: stats.reused,
                  frameMs: stats.frameMs,
                });
              }
              const completion = new Promise((resolve) => (done = resolve));
              let lastEdit = 0;
              for (let edit = 0; edit < 20; edit++) {
                lastEdit = performance.now();
                viewer.setMarkdown(source + "\nTyping " + edit);
                await new Promise((resolve) => setTimeout(resolve, 16));
              }
              await completion;
              while (viewer.reader.markview.stats().pending)
                await new Promise((resolve) => requestAnimationFrame(resolve));
              edits.push({
                label,
                burst: 20,
                lastEditToCompleteMs: performance.now() - lastEdit,
              });
            }
          }
          const scrolling = [];
          if (profileUpdates) {
            let previousFrame = performance.now();
            for (let frame = 0; frame < 120; frame++) {
              await new Promise((resolve) => requestAnimationFrame(resolve));
              const now = performance.now();
              scrolling.push({
                intervalMs: now - previousFrame,
                frameMs: viewer.reader.markview.stats().frameMs,
              });
              previousFrame = now;
              viewer.scrollTo(frame * 12);
            }
          }
          viewer.destroy();
          fonts.destroy();
          return {
            fetchMs: fetched - start,
            wasmMs: initialized - fetched,
            fontSetMs: decoded - initialized,
            mountRenderMs: finish - decoded,
            totalMs: finish - start,
            edits,
            scrolling,
          };
        },
        {
          urls: files.map((file) => "/font/" + full.indexOf(file)),
          markdown,
          profileUpdates: name === "without-emoji" && run === 0,
        },
      );
      results.push({
        name,
        run,
        bytes: (
          await Promise.all(files.map(async (file) => (await stat(file)).size))
        ).reduce((a, b) => a + b, 0),
        ...result,
      });
      console.log(JSON.stringify(results.at(-1)));
      await context.close();
    }
  }
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
await mkdir(path.join(root, "artifacts/vscode"), { recursive: true });
const { writeFile } = await import("node:fs/promises");
await writeFile(
  path.join(root, "artifacts/vscode/startup-profile.json"),
  JSON.stringify(
    {
      scanMs,
      source:
        "Chromium localhost, cold contexts, warm OS file cache; not VS Code Webview transport",
      results,
    },
    null,
    2,
  ),
);
