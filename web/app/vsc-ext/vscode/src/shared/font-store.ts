import { promises as fs } from "node:fs";
import path from "node:path";
import type { Family } from "./types";

// Install a complete family only after the native downloader has validated it.
export async function installFamily(
  cache: string,
  family: Family,
  download: (directory: string) => Promise<void>,
  signal?: AbortSignal,
): Promise<void> {
  signal?.throwIfAborted();
  await fs.mkdir(cache, { recursive: true });
  const staging = await fs.mkdtemp(path.join(path.dirname(cache), "download-"));
  const target = path.join(cache, Buffer.from(family.id).toString("hex"));
  const backup = `${staging}-backup`;
  try {
    await download(staging);
    signal?.throwIfAborted();
    await fs.writeFile(
      path.join(staging, "metadata.json"),
      JSON.stringify({
        ...family,
        downloadedAt: new Date().toISOString(),
      }),
    );
    let previous = false;
    try {
      await fs.rename(target, backup);
      previous = true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    try {
      signal?.throwIfAborted();
      await fs.rename(staging, target);
    } catch (error) {
      if (previous) await fs.rename(backup, target);
      throw error;
    }
    await fs.rm(backup, { recursive: true, force: true });
  } finally {
    await fs.rm(staging, { recursive: true, force: true });
  }
}
