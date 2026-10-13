import * as vscode from "vscode";
import path from "node:path";
import { FontClient } from "./font-client";
import { FontIndex } from "./fonts";
import type { Template } from "./types";

export class Services extends FontClient implements vscode.Disposable {
  readonly changed = new vscode.EventEmitter<void>();
  readonly fonts: FontIndex;
  constructor(readonly context: vscode.ExtensionContext) {
    super(
      context.asAbsolutePath(
        path.join(
          "bin",
          process.platform === "win32" ? "markview.exe" : "markview",
        ),
      ),
      path.join(context.globalStorageUri.fsPath, "fonts"),
    );
    this.fonts = new FontIndex(
      this.cache,
      path.join(context.globalStorageUri.fsPath, "font-index.json"),
    );
  }

  directories(): string[] {
    return vscode.workspace
      .getConfiguration("markview")
      .get<string[]>("fontDirectories", []);
  }
  readingTemplate(): Template {
    const file = vscode.workspace
      .getConfiguration("markview")
      .get<string>("previewStyleFile", "");
    return file
      ? { id: "custom", name: path.basename(file), path: file }
      : { id: "default", name: "阅读字体" };
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
  override async downloadFamily(
    ...args: Parameters<FontClient["downloadFamily"]>
  ) {
    await super.downloadFamily(...args);
    this.changed.fire();
  }
  dispose() {
    this.changed.dispose();
  }
}
