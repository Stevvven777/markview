import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, copyFile, mkdir } from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { FontIndex } from "../dist/fonts.js";
import { runNative } from "../dist/process.js";

test("font index reads internal family names, TTC faces and fallback candidates", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "markview-font-test-"));
  try {
    const index = new FontIndex(dir, path.join(dir, "index.json"));
    const fixtures = path.resolve("../../../crates/markview-web/tests/fonts");
    await index.refresh([], [path.join(fixtures, "Noto-subset.ttc")]);
    assert.equal(index.records.length, 1);
    assert.ok(index.records[0].names.includes("Noto Serif"));
    assert.ok(index.records[0].names.includes("Noto Sans"));
    const catalog = {
      templates: [],
      families: [],
      definitions: [
        { id: "body", lookfor: ["Missing", "Noto Serif"] },
        { id: "other", lookfor: ["Missing"] },
      ],
    };
    const statuses = index.statuses(catalog);
    assert.equal(statuses[0].candidate, "Noto Serif");
    assert.equal(statuses[1].match, undefined);
    assert.equal(index.paths(catalog).length, 1);
    await index.refresh([], [path.join(fixtures, "Noto-subset.ttc")]);
    assert.equal(index.records.length, 1);
    const downloaded = path.join(dir, "downloaded.ttc");
    await copyFile(path.join(fixtures, "Noto-subset.ttc"), downloaded);
    await index.refresh(
      [],
      [path.join(fixtures, "Noto-subset.ttc"), downloaded],
    );
    assert.equal(index.statuses(catalog)[0].downloaded, true);
    assert.deepEqual(index.paths(catalog), [downloaded]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("directory scanning skips invalid files and invalidates cached names after replacement", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "markview-font-roots-"));
  try {
    const extra = path.join(dir, "extra");
    await mkdir(extra);
    const face = path.join(extra, "fixture.otf");
    await copyFile(
      path.resolve(
        "../../../crates/markview-layout/tests/fonts/NotoSerif-Regular-subset.otf",
      ),
      face,
    );
    await writeFile(path.join(extra, "broken.ttf"), "not a font");
    const index = new FontIndex(
      path.join(dir, "cache"),
      path.join(dir, "index.json"),
    );
    await index.refresh([extra]);
    const catalog = {
      templates: [],
      families: [],
      definitions: [{ id: "serif", lookfor: ["Noto Serif"] }],
    };
    assert.equal(index.statuses(catalog)[0].source, "directory");
    assert.equal(index.statuses(catalog)[0].match.path, face);
    await writeFile(face, "invalid replacement");
    await index.refresh([extra], [face]);
    assert.equal(index.statuses(catalog)[0].match, undefined);
    await rm(face);
    await index.refresh([extra], []);
    assert.equal(index.records.length, 0);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("process transport preserves stdin and parses progress independently from errors", async () => {
  const result = await runNative(
    process.execPath,
    [
      "-e",
      'process.stdin.on("data", b => process.stdout.write(JSON.stringify({ type:"progress", message:b.toString() })+"\\n"))',
    ],
    "未保存\r\ntext",
  );
  assert.equal(JSON.parse(result).message, "未保存\r\ntext");
  await assert.rejects(
    runNative(process.execPath, [
      "-e",
      'console.error("bad template");process.exit(2)',
    ]),
    /bad template/,
  );
});

test("cancellation waits for child exit and reports AbortError", async () => {
  const controller = new AbortController();
  const result = runNative(
    process.execPath,
    ["-e", "setInterval(()=>{},1000)"],
    "",
    controller.signal,
  );
  setTimeout(() => controller.abort(), 100);
  await assert.rejects(result, { name: "AbortError" });
});
