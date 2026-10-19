import { test } from "node:test";
import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { exportSnapshot } from "../dist/export-snapshot.js";
import { FontClient } from "../dist/font-client.js";
import { FontIndex } from "../dist/fonts.js";

test("native exports snapshot text to PDF/PNG and preserves output on failure or cancellation", async () => {
  assert.ok(
    process.env.MARKVIEW_BINARY,
    "Set MARKVIEW_BINARY to the current native CLI",
  );
  const root = await fs.mkdtemp(
    path.join(os.tmpdir(), "markview-export-test-"),
  );
  try {
    const services = new FontClient(
      process.env.MARKVIEW_BINARY,
      path.join(root, "fonts"),
    );
    services.fonts = new FontIndex(
      services.cache,
      path.join(root, "index.json"),
    );
    services.directories = () => [];
    const template = { id: "default", name: "Default" };
    for (const format of ["pdf", "png"]) {
      const output = path.join(root, `result.${format}`);
      const events = [];
      await exportSnapshot(
        services,
        "# Unsaved snapshot\n\nLatest editor contents.\n",
        root,
        format,
        output,
        template,
        undefined,
        (event) => events.push(event),
      );
      const bytes = await fs.readFile(output);
      assert.ok(bytes.length > 100);
      assert.equal(
        bytes.subarray(0, 4).toString("hex"),
        format === "pdf" ? "25504446" : "89504e47",
      );
      assert.ok(events.some((event) => event.type === "done"));
      await assert.rejects(
        exportSnapshot(
          services,
          "replacement",
          root,
          format,
          output,
          template,
          AbortSignal.abort(),
        ),
        { name: "AbortError" },
      );
      assert.deepEqual(await fs.readFile(output), bytes);
      await assert.rejects(
        exportSnapshot(services, "replacement", root, format, output, {
          id: "missing-style",
          name: "Bad template",
        }),
      );
      assert.deepEqual(await fs.readFile(output), bytes);
    }
    assert.equal(
      (await fs.readdir(root)).some((name) =>
        name.startsWith(".markview-export-"),
      ),
      false,
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
