import { promises as fs } from "node:fs";
import path from "node:path";
import { runNative } from "./process";
import type { FontClient } from "./font-client";
import type { Template, Progress } from "./types";

export async function exportSnapshot(
  services: FontClient & { directories(): string[] },
  text: string,
  base: string,
  format: "pdf" | "png",
  output: string,
  template: Template,
  signal?: AbortSignal,
  progress?: (event: Progress) => void,
  settings = { fontSize: 16, scale: 2, trusted: false },
): Promise<void> {
  signal?.throwIfAborted();
  await fs.mkdir(services.cache, { recursive: true });
  const directories = [...services.directories(), services.cache];
  const staging = await fs.mkdtemp(
    path.join(path.dirname(output), ".markview-export-"),
  );
  try {
    const target = path.join(staging, `result.${format}`);
    await runNative(
      services.binary,
      [
        "export",
        "--stdin",
        "--format",
        format,
        "--output",
        target,
        "--base-dir",
        base,
        "--document-trust",
        settings.trusted ? "trusted" : "untrusted",
        "--font-size",
        String(settings.fontSize),
        "--scale",
        String(settings.scale),
        ...(template.path
          ? ["--style-file", template.path]
          : template.id !== "default"
            ? ["--style", template.id]
            : []),
        ...directories.flatMap((directory) => ["--fonts", directory]),
      ],
      text,
      signal,
      progress,
    );
    signal?.throwIfAborted();
    await fs.rename(target, output);
  } finally {
    await fs.rm(staging, { recursive: true, force: true });
  }
}
