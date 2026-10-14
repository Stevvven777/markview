import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile, readdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { installFamily } from "../dist/font-store.js";

test("family replacement preserves old files on failure/cancellation and commits metadata", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "markview-font-store-"));
  const cache = path.join(dir, "fonts");
  const family = {
    id: "../family",
    lookfor: ["Example"],
    sources: 1,
    license: "OFL-1.1",
  };
  const output = path.join(
    cache,
    Buffer.from(family.id).toString("hex"),
    "face.otf",
  );
  try {
    await installFamily(cache, family, (stage) =>
      writeFile(path.join(stage, "face.otf"), "old"),
    );
    await assert.rejects(
      installFamily(cache, family, async (stage) => {
        await writeFile(path.join(stage, "face.otf"), "partial");
        throw new Error("download failed");
      }),
      /download failed/,
    );
    assert.equal(await readFile(output, "utf8"), "old");
    const controller = new AbortController();
    await assert.rejects(
      installFamily(
        cache,
        family,
        async () => controller.abort(),
        controller.signal,
      ),
      { name: "AbortError" },
    );
    assert.equal(await readFile(output, "utf8"), "old");
    await installFamily(cache, family, (stage) =>
      writeFile(path.join(stage, "face.otf"), "new"),
    );
    assert.equal(await readFile(output, "utf8"), "new");
    const metadata = JSON.parse(
      await readFile(path.join(path.dirname(output), "metadata.json"), "utf8"),
    );
    assert.equal(metadata.license, family.license);
    assert.deepEqual(await readdir(dir), ["fonts"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("custom template batches use one snapshot and clean it up after failure", async () => {
  const { FontClient } = await import("../dist/font-client.js");
  const dir = await mkdtemp(path.join(os.tmpdir(), "markview-template-test-"));
  const original = path.join(dir, "style.mvss.toml");
  const client = new FontClient("unused", dir);
  let snapshot;
  try {
    await writeFile(original, "original");
    await assert.rejects(
      client.withTemplate(
        { id: "custom", name: "Custom", path: original },
        async (template) => {
          snapshot = template.path;
          await writeFile(original, "changed");
          assert.equal(await readFile(snapshot, "utf8"), "original");
          throw new Error("batch failed");
        },
      ),
      /batch failed/,
    );
    await assert.rejects(readFile(snapshot), { code: "ENOENT" });
    assert.equal(await readFile(original, "utf8"), "changed");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
