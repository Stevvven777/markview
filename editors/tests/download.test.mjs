import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { runNative } from "../dist/process.js";
import { FontIndex } from "../dist/fonts.js";
import { downloadFixture } from "../dist/download-fixture.js";

test("native font download validates local responses, retries and cancels in flight", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "markview-download-test-"));
  const fixture = await downloadFixture(
    dir,
    path.resolve(
      "../crates/markview-core/tests/fonts/NotoSerif-Regular-subset.otf",
    ),
  );
  try {
    const style = fixture.template;
    const binary =
      process.env.MARKVIEW_BINARY ??
      path.resolve(
        "../target/release",
        process.platform === "win32" ? "markview.exe" : "markview",
      );
    const cache = path.join(dir, "fonts");
    const args = [
      "export",
      "--download",
      "fixture",
      "--cache",
      cache,
      "--style-file",
      style,
    ];
    await assert.rejects(runNative(binary, args), /font|face|invalid|Font/i);
    fixture.serve("valid");
    const progress = [];
    await runNative(binary, args, "", undefined, (event) =>
      progress.push(event),
    );
    assert.ok(progress.some((event) => event.type === "progress"));
    const index = new FontIndex(cache, path.join(dir, "index.json"));
    await index.refresh(
      [],
      (await readdir(cache))
        .filter((f) => f.endsWith(".otf"))
        .map((f) => path.join(cache, f)),
    );
    assert.ok(
      index.records.some((record) => record.names.includes("Noto Serif")),
    );
    const controller = new AbortController();
    fixture.serve("slow", controller);
    await assert.rejects(
      runNative(
        binary,
        [
          "export",
          "--download",
          "fixture",
          "--cache",
          path.join(dir, "cancelled"),
          "--style-file",
          style,
        ],
        "",
        controller.signal,
      ),
      { name: "AbortError" },
    );
    assert.ok(
      !(await readdir(path.join(dir, "cancelled"))).some((f) =>
        f.endsWith(".otf"),
      ),
    );
  } finally {
    await fixture.dispose();
    await rm(dir, { recursive: true, force: true });
  }
});
