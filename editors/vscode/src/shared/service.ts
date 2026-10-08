import os from "node:os";
import * as vscode from "vscode";
import path from "node:path";
import { promises as fs } from "node:fs";
import { needsEmojiFont } from "./preview-fonts";
import { FontPanel } from "./font-panel";
import { FontIndex } from "./fonts";
import { runNative } from "./process";
import type { Catalog, Template, Progress } from "./types";

export class Services implements vscode.Disposable {
  readonly changed = new vscode.EventEmitter<void>();
  readonly fonts: FontIndex;
  readonly output = vscode.window.createOutputChannel("Markview");
  readonly binary: string;
  private fontPanel?: FontPanel;
  constructor(readonly context: vscode.ExtensionContext) {
    this.binary = context.asAbsolutePath(
      path.join(
        "bin",
        process.platform === "win32" ? "markview.exe" : "markview",
      ),
    );
    this.fonts = new FontIndex(
      path.join(context.globalStorageUri.fsPath, "fonts"),
      path.join(context.globalStorageUri.fsPath, "font-index.json"),
    );
  }
  directories(): string[] {
    return vscode.workspace
      .getConfiguration("markview")
      .get<string[]>("fontDirectories", []);
  }
  templateArgs(template?: Template): string[] {
    return template?.path
      ? ["--style-file", template.path]
      : template && template.id !== "default"
        ? ["--style", template.id]
        : [];
  }
  async catalog(template?: Template, destination = "pdf"): Promise<Catalog> {
    if (
      (!template || template.id === "default") &&
      !template?.path &&
      destination === "ui"
    )
      return JSON.parse(
        await fs.readFile(
          this.context.asAbsolutePath("resources/ui-fonts.json"),
          "utf8",
        ),
      );
    const raw = await runNative(this.binary, [
      "export",
      "--catalog",
      "--destination",
      destination,
      ...this.templateArgs(template),
    ]);
    return JSON.parse(raw.trim());
  }
  async refresh() {
    await this.fonts.refresh(this.directories());
    this.changed.fire();
  }
  readingTemplate(): Template {
    const file = vscode.workspace
      .getConfiguration("markview")
      .get<string>("previewStyleFile", "");
    return file
      ? { id: "custom", name: path.basename(file), path: file }
      : { id: "default", name: "阅读字体" };
  }
  async previewStylesheet(): Promise<string | undefined> {
    const file = this.readingTemplate().path;
    return file ? fs.readFile(file, "utf8") : undefined;
  }
  async previewFonts(markdown?: string): Promise<string[]> {
    const catalog = await this.catalog(this.readingTemplate(), "ui");
    await this.fonts.refresh(this.directories());
    if (markdown !== undefined && !needsEmojiFont(markdown))
      catalog.definitions = catalog.definitions.filter(
        ({ id }) => id !== "emoji",
      );
    return this.fonts.paths(catalog);
  }
  async pickTemplate(): Promise<Template | undefined> {
    const { templates } = await this.catalog();
    const choice = await vscode.window.showQuickPick(
      [
        {
          label: "默认导出样式",
          template: { id: "default", name: "Default" } as Template,
        },
        ...templates.map((template) => ({
          label: template.name,
          description: template.id,
          template,
        })),
        { label: "自定义 .mvss.toml…", template: undefined },
      ],
      { title: "选择导出模板" },
    );
    if (!choice) return;
    if (choice.template) return choice.template;
    const files = await vscode.window.showOpenDialog({
      canSelectMany: false,
      filters: { MVSS: ["mvss.toml", "toml"] },
      title: "选择 MVSS 模板",
    });
    return files?.[0]
      ? {
          id: "custom",
          name: path.basename(files[0].fsPath),
          path: files[0].fsPath,
        }
      : undefined;
  }
  exportSettings() {
    const settings = vscode.workspace.getConfiguration("markview");
    return {
      fontSize: settings.get("exportFontSize", 16),
      scale: settings.get("exportScale", 2),
      directories: [...this.directories()],
    };
  }
  async withTemplate<T>(
    template: Template,
    operation: (snapshot: Template) => Promise<T>,
  ): Promise<T> {
    if (!template.path) return operation(template);
    const source = await fs.readFile(template.path);
    const staging = await fs.mkdtemp(
      path.join(os.tmpdir(), "markview-template-"),
    );
    const file = path.join(staging, "template.mvss.toml");
    try {
      await fs.writeFile(file, source);
      return await operation({ ...template, path: file });
    } finally {
      await fs.rm(staging, { recursive: true, force: true });
    }
  }
  async exportSnapshot(
    text: string,
    base: string,
    format: "pdf" | "png",
    output: string,
    template?: Template,
    signal?: AbortSignal,
    progress?: (event: Progress) => void,
    settings = this.exportSettings(),
  ): Promise<void> {
    signal?.throwIfAborted();
    const catalog = await this.catalog(template);
    await this.fonts.refresh(settings.directories);
    const files = this.fonts.paths(catalog);
    const staging = await fs.mkdtemp(
      path.join(path.dirname(output), ".markview-export-"),
    );
    const target = path.join(staging, `result.${format}`);
    try {
      await fs.mkdir(this.fonts.cache, { recursive: true });
      await runNative(
        this.binary,
        [
          "export",
          "--format",
          format,
          "--output",
          target,
          "--stdin",
          "--base-dir",
          base,
          "--font-size",
          String(settings.fontSize),
          "--scale",
          String(settings.scale),
          ...this.templateArgs(template),
          ...files.flatMap((file) => ["--font-file", file]),
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
  async downloadFamily(
    template: Template,
    destination: string,
    id: string,
    signal?: AbortSignal,
    progress?: (event: Progress) => void,
  ) {
    await fs.mkdir(this.fonts.cache, { recursive: true });
    const staging = await fs.mkdtemp(
      path.join(path.dirname(this.fonts.cache), "download-"),
    );
    try {
      await runNative(
        this.binary,
        [
          "export",
          "--download",
          id,
          "--cache",
          staging,
          "--destination",
          destination,
          ...this.templateArgs(template),
        ],
        "",
        signal,
        progress,
      );
      signal?.throwIfAborted();
      const catalog = await this.catalog(template, destination);
      await fs.writeFile(
        path.join(staging, "metadata.json"),
        JSON.stringify({
          ...catalog.families.find((family) => family.id === id),
          downloadedAt: new Date().toISOString(),
        }),
      );
      const target = path.join(
        this.fonts.cache,
        Buffer.from(id).toString("hex"),
      );
      const backup = `${staging}-backup`;
      let hadPrevious = false;
      try {
        await fs.rename(target, backup);
        hadPrevious = true;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
      try {
        signal?.throwIfAborted();
        await fs.rename(staging, target);
      } catch (error) {
        if (hadPrevious) await fs.rename(backup, target);
        throw error;
      }
      await fs.rm(backup, { recursive: true, force: true });
      await this.refresh();
    } finally {
      await fs.rm(staging, { recursive: true, force: true });
    }
  }
  async manageFonts(template?: Template, destination = "pdf"): Promise<void> {
    if (!this.fontPanel || this.fontPanel.disposed)
      this.fontPanel = new FontPanel(this);
    await this.fontPanel.show(template, destination);
  }
  dispose() {
    this.fontPanel?.dispose();
    this.changed.dispose();
    this.output.dispose();
  }
}
