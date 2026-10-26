import * as vscode from "vscode";
import path from "node:path";
import { readFile } from "node:fs/promises";
import { parse, type ParseError } from "jsonc-parser/lib/esm/main.js";
import type { Template } from "./types";

export class TemplateSettings implements vscode.Disposable {
  private files = new Map<string, Promise<Record<string, unknown> | null>>();
  private watchers = new Map<string, vscode.FileSystemWatcher>();

  constructor(private changed: () => void) {}

  watch(file: string) {
    if (this.watchers.has(file)) return;
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(path.dirname(file), path.basename(file)),
    );
    this.watchers.set(file, watcher);
    for (const event of [
      watcher.onDidChange,
      watcher.onDidCreate,
      watcher.onDidDelete,
    ])
      event(() => {
        this.files.clear();
        this.changed();
      });
  }

  private read(file: string) {
    this.watch(path.dirname(file));
    this.watch(file);
    let pending = this.files.get(file);
    if (!pending) {
      pending = readFile(file, "utf8").then(
        (source) => {
          const errors: ParseError[] = [];
          const values = parse(source, errors, {
            allowTrailingComma: true,
          });
          if (
            errors.length ||
            !values ||
            typeof values !== "object" ||
            Array.isArray(values)
          )
            throw new Error(`Invalid Markview settings: ${file}`);
          return values as Record<string, unknown>;
        },
        (error: NodeJS.ErrnoException) => {
          if (["ENOENT", "ENOTDIR"].includes(error.code ?? "")) return {};
          if (["EACCES", "EPERM"].includes(error.code ?? "")) return null;
          throw error;
        },
      );
      this.files.set(file, pending);
    }
    return pending;
  }

  async resolve(uri: vscode.Uri | undefined, destination: string) {
    if (uri?.scheme !== "file") return;
    let directory = path.dirname(uri.fsPath);
    const prefix = destination === "ui" ? "preview" : "export";
    for (;;) {
      const file = path.join(directory, ".vscode", "settings.json");
      const values = await this.read(file);
      if (values === null) return;
      const fileKey = `markview.${prefix}StyleFile`;
      const idKey = `markview.${prefix}Template`;
      if (fileKey in values || idKey in values) {
        for (const key of [fileKey, idKey])
          if (key in values && typeof values[key] !== "string")
            throw new Error(`Expected a string for ${key}: ${file}`);
        const custom = values[fileKey] as string | undefined;
        const id = (values[idKey] as string | undefined) || "default";
        const template: Template = custom
          ? {
              id: "custom",
              name: path.basename(custom),
              path: path.resolve(directory, custom),
            }
          : { id, name: id };
        template.source = file;
        return template;
      }
      const parent = path.dirname(directory);
      if (parent === directory) return;
      directory = parent;
    }
  }

  dispose() {
    this.watchers.forEach((watcher) => watcher.dispose());
    this.watchers.clear();
    this.files.clear();
  }
}
