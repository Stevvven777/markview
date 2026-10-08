import * as vscode from "vscode";
import { randomBytes } from "node:crypto";
import type { Services } from "./service";
import type { Family, FontStatus, Template } from "./types";

export interface FontPanelState {
  destination: string;
  template: Template;
  rows: FontStatus[];
  busy: boolean;
}

export class FontPanel implements vscode.Disposable {
  readonly panel: vscode.WebviewPanel;
  disposed = false;
  private destination = "ui";
  private template: Template = { id: "default", name: "阅读字体" };
  private rows: FontStatus[] = [];
  private generation = 0;
  private download?: AbortController;
  private subscriptions: vscode.Disposable[] = [];

  constructor(private services: Services) {
    const media = vscode.Uri.joinPath(services.context.extensionUri, "media");
    this.panel = vscode.window.createWebviewPanel(
      "markview.fonts",
      "Markview 字体管理",
      vscode.ViewColumn.Active,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [media],
      },
    );
    const nonce = randomBytes(16).toString("hex");
    const uri = (file: string) =>
      this.panel.webview.asWebviewUri(vscode.Uri.joinPath(media, file));
    this.panel.webview.html = `<!doctype html><html lang="zh"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${this.panel.webview.cspSource}; style-src ${this.panel.webview.cspSource}; script-src 'nonce-${nonce}';"><link rel="stylesheet" href="${uri("fonts.css")}"></head><body>
<header><img src="${uri("brand.svg")}" alt="" width="32" height="32"><div><h1>字体管理</h1><p>查看本地字体，补齐阅读和导出模板需要的字体。</p></div></header>
<nav aria-label="字体用途"><button data-action="ui" aria-pressed="true">阅读字体</button><button data-action="pdf" aria-pressed="false">导出模板字体</button></nav>
<section class="tools"><h2 id="template">阅读字体</h2><button data-action="template" hidden>选择模板…</button><button data-action="download-all">下载全部缺失字体</button><button data-action="refresh">刷新</button><button data-action="directory">添加字体目录…</button></section>
<div class="notice"><p id="status" role="status">正在检查本地字体…</p><progress id="progress" hidden></progress><button data-action="cancel" hidden>取消下载</button></div>
<main id="fonts" aria-label="字体列表"></main><script nonce="${nonce}" src="${uri("fonts.js")}"></script></body></html>`;
    this.subscriptions.push(
      this.panel.webview.onDidReceiveMessage((message) => {
        void this.handle(message).catch((error) =>
          this.notify(`操作失败：${String(error)}`, false),
        );
      }),
      services.changed.event(() => {
        void this.refresh().catch((error) => this.notify(String(error), false));
      }),
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("markview.fontDirectories"))
          void this.refresh().catch((error) =>
            this.notify(String(error), false),
          );
      }),
      this.panel.onDidDispose(() => {
        this.disposed = true;
        this.generation++;
        this.download?.abort();
        this.subscriptions.forEach((subscription) => subscription.dispose());
      }),
    );
  }
  async show(template?: Template, destination = "pdf") {
    this.panel.reveal();
    if (this.download) return;
    this.destination = destination;
    this.template =
      template ??
      (destination === "ui"
        ? this.services.readingTemplate()
        : { id: "default", name: "默认导出样式" });
    await this.refresh();
  }
  private notify(message: string, busy: boolean, fraction?: number) {
    if (!this.disposed)
      void this.panel.webview.postMessage({
        type: "progress",
        message,
        busy,
        fraction,
      });
  }
  private async refresh() {
    if (this.disposed) return;
    const generation = ++this.generation;
    const catalog = await this.services.catalog(
      this.template,
      this.destination,
    );
    await this.services.fonts.refresh(this.services.directories());
    if (this.disposed || generation !== this.generation) return;
    this.rows = this.services.fonts.statuses(catalog);
    await this.panel.webview.postMessage({
      type: "state",
      destination: this.destination,
      template: this.template,
      rows: this.rows,
      busy: !!this.download,
    });
  }
  private async handle(message: {
    action?: string;
    id?: string;
    family?: string;
    families?: Record<string, string>;
  }) {
    if (message.action === "cancel") {
      this.download?.abort();
      return;
    }
    if (this.download || this.disposed) return;
    switch (message.action) {
      case "ready":
      case "refresh":
        await this.refresh();
        break;
      case "ui":
      case "pdf":
        await this.show(undefined, message.action);
        break;
      case "template": {
        const template = await this.services.pickTemplate();
        if (template) await this.show(template, "pdf");
        break;
      }
      case "directory": {
        const folders = await vscode.window.showOpenDialog({
          canSelectFolders: true,
          canSelectFiles: false,
          canSelectMany: false,
          title: "添加字体目录",
        });
        if (!folders?.[0]) return;
        const directories = [
          ...new Set([...this.services.directories(), folders[0].fsPath]),
        ];
        await vscode.workspace
          .getConfiguration("markview")
          .update(
            "fontDirectories",
            directories,
            vscode.ConfigurationTarget.Global,
          );
        break;
      }
      case "download-all": {
        const missing = this.rows.filter((row) => !row.match);
        const selected = missing.map(
          (row) =>
            row.families.find(
              (family) =>
                family.sources > 0 &&
                family.id === message.families?.[row.definition.id],
            ) ?? row.families.find((family) => family.sources > 0),
        );
        const families = [
          ...new Map(
            selected
              .filter((family): family is Family => !!family)
              .map((family) => [family.id, family]),
          ).values(),
        ];
        await this.downloadFamilies(
          families,
          selected.filter((family) => !family).length,
        );
        break;
      }
      case "download": {
        const row = this.rows.find(
          (row) => row.definition.id === message.id && !row.match,
        );
        const family = row?.families.find(
          (family) => family.id === message.family && family.sources > 0,
        );
        if (!family) return;
        await this.downloadFamilies([family]);
        break;
      }
    }
  }
  private async downloadFamilies(families: Family[], unavailable = 0) {
    if (!families.length) {
      this.notify(
        unavailable
          ? `${unavailable} 项缺失字体没有下载来源，请添加本地字体目录。`
          : "没有需要下载的字体。",
        false,
      );
      return;
    }
    const controller = (this.download = new AbortController());
    const failed: string[] = [];
    let completed = 0;
    this.notify(`准备下载 ${families.length} 个字体家族…`, true, 0);
    try {
      await this.services.withTemplate(this.template, async (template) => {
        for (const [index, family] of families.entries()) {
          if (controller.signal.aborted) break;
          const label = `${index + 1}/${families.length} ${family.lookfor[0]}`;
          try {
            await this.services.downloadFamily(
              template,
              this.destination,
              family.id,
              controller.signal,
              (event) =>
                this.notify(
                  `${label} · ${event.message ?? event.phase ?? "正在下载…"}`,
                  true,
                  (index + (event.fraction ?? 0)) / families.length,
                ),
            );
            completed++;
          } catch (error) {
            if (controller.signal.aborted) break;
            failed.push(`${family.lookfor[0]}：${String(error)}`);
          }
        }
      });
    } finally {
      this.download = undefined;
      await this.refresh();
    }
    const summary = [
      controller.signal.aborted
        ? "下载已取消"
        : `下载完成：${completed} 个字体家族`,
    ];
    if (failed.length)
      summary.push(`下载失败：${failed.join("；")}。可重新点击下载按钮重试。`);
    if (unavailable) summary.push(`${unavailable} 项缺失字体没有下载来源`);
    this.notify(summary.join("；"), false);
  }
  dispose() {
    this.panel.dispose();
  }
}
