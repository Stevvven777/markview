import { promises as fs } from "node:fs";
import path from "node:path";
import os from "node:os";
import * as fontkit from "fontkit";
import getSystemFonts from "get-system-fonts";
import type { Catalog, FontRecord, FontStatus } from "./types";

const normalize = (name: string) => name.trim().toLocaleLowerCase("en-US");
export class FontIndex {
  records: FontRecord[] = [];
  private pending?: Promise<void>;
  private roots: string[] = [];
  private source(file: string) {
    return this.roots.findIndex((root) => {
      const relative = path.relative(root, file);
      return !path.isAbsolute(relative) && relative.split(path.sep)[0] !== "..";
    });
  }
  constructor(
    readonly cache: string,
    private indexFile: string,
  ) {}
  async refresh(extra: string[] = [], only?: string[]): Promise<void> {
    const scan = (this.pending ?? Promise.resolve())
      .catch(() => {})
      .then(() => this.scan(extra, only));
    this.pending = scan;
    try {
      await scan;
    } finally {
      if (this.pending === scan) this.pending = undefined;
    }
  }

  private async scan(extra: string[], only?: string[]) {
    await fs.mkdir(this.cache, { recursive: true });
    let previous: FontRecord[] = [];
    try {
      const parsed = JSON.parse(await fs.readFile(this.indexFile, "utf8"));
      if (Array.isArray(parsed)) previous = parsed;
    } catch {
      /* An absent or obsolete index is rebuilt. */
    }
    const cached = new Map(previous.map((item) => [item.path, item]));
    const paths =
      only ??
      (await getSystemFonts({
        additionalFolders: [
          this.cache,
          ...extra,
          ...(process.platform === "win32"
            ? [
                path.join(
                  process.env.LOCALAPPDATA ??
                    path.join(os.homedir(), "AppData", "Local"),
                  "Microsoft",
                  "Windows",
                  "Fonts",
                ),
              ]
            : []),
        ],
      }));
    this.roots = [this.cache, ...extra].map((root) => path.resolve(root));
    const records: FontRecord[] = [];
    for (const file of new Set(paths)) {
      try {
        const stat = await fs.stat(file),
          old = cached.get(file);
        if (old?.size === stat.size && old.mtime === stat.mtimeMs) {
          records.push(old);
          continue;
        }
        const font = fontkit.openSync(file);
        const faces = "fonts" in font ? font.fonts : [font];
        const names = new Set<string>(),
          weights = new Set<number>(),
          styles = new Set<string>();
        for (const face of faces) {
          names.add(face.familyName);
          // Keep localized family names as well as the default spelling.
          const raw = (
            face as unknown as {
              name?: { records?: Record<string, Record<string, string>> };
            }
          ).name?.records;
          for (const key of [
            "fontFamily",
            "preferredFamily",
            "typographicFamily",
          ])
            for (const name of Object.values(raw?.[key] ?? {}))
              if (typeof name === "string") names.add(name);
          const os2 = (
            face as unknown as { "OS/2"?: { usWeightClass?: number } }
          )["OS/2"];
          weights.add(os2?.usWeightClass ?? 400);
          styles.add(face.subfamilyName);
        }
        records.push({
          path: file,
          size: stat.size,
          mtime: stat.mtimeMs,
          names: [...names],
          weights: [...weights],
          styles: [...styles],
        });
      } catch {
        /* Unreadable fonts cannot be offered to the renderer. */
      }
    }
    this.records = records.sort((a, b) => {
      const rank = (record: FontRecord) => {
        const source = this.source(record.path);
        return source < 0 ? this.roots.length : source;
      };
      return rank(a) - rank(b) || a.path.localeCompare(b.path);
    });
    await fs.mkdir(path.dirname(this.indexFile), { recursive: true });
    await fs.writeFile(this.indexFile, JSON.stringify(records));
  }
  statuses(catalog: Catalog): FontStatus[] {
    return catalog.definitions.map((definition) => {
      let match: FontRecord | undefined, candidate: string | undefined;
      for (const name of definition.lookfor) {
        match = this.records.find((record) =>
          record.names.some((family) => normalize(family) === normalize(name)),
        );
        if (match) {
          candidate = name;
          break;
        }
      }
      const families = catalog.families.filter((family) =>
        family.lookfor.some((name) =>
          definition.lookfor.some(
            (candidate) => normalize(candidate) === normalize(name),
          ),
        ),
      );
      return {
        definition,
        source:
          !match || this.source(match.path) < 0
            ? "system"
            : this.source(match.path) === 0
              ? "cache"
              : "directory",
        match,
        candidate,
        downloaded:
          !!match &&
          path.relative(this.cache, match.path).split(path.sep)[0] !== ".." &&
          !path.isAbsolute(path.relative(this.cache, match.path)),
        families,
      };
    });
  }
  paths(catalog: Catalog): string[] {
    const statuses = this.statuses(catalog);
    return [
      ...new Set(
        statuses.flatMap(({ match, candidate }) =>
          !match
            ? []
            : this.records
                .filter(
                  (record) =>
                    this.source(record.path) === this.source(match.path) &&
                    record.names.some(
                      (name) => normalize(name) === normalize(candidate!),
                    ),
                )
                .map((record) => record.path),
        ),
      ),
    ];
  }
}
