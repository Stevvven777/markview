import * as vscode from "vscode";
import path from "node:path";
import { resourceDirectory } from "./shared/resources";
import { Services } from "./shared/service";
import type { Template } from "./shared/types";

export function activate(context: vscode.ExtensionContext): Services {
  const services = new Services(context);
  context.subscriptions.push(services);
  async function exportDocument(
    format?: "pdf" | "png",
    withTemplate = false,
    editor = vscode.window.activeTextEditor,
  ) {
    const document = editor?.document;
    if (!document || document.languageId !== "markdown") {
      await vscode.window.showWarningMessage("请先打开 Markdown 文档");
      return;
    }
    const text = document.getText();
    const settings = services.exportSettings();
    const template: Template | undefined = withTemplate
      ? await services.pickTemplate()
      : {
          id: vscode.workspace
            .getConfiguration("markview", document.uri)
            .get("exportTemplate", "default"),
          name: "Default",
        };
    if (!template) return;
    return services.withTemplate(template, async (template) => {
      const chosen =
        format ??
        (await vscode.window.showQuickPick(["PDF", "PNG"], {
          title: "选择导出格式",
        }));
      if (!chosen) return;
      const extension = chosen.toLowerCase();
      const base = await resourceDirectory(document, text);
      if (!base) return;
      const stem = document.isUntitled
        ? "Untitled"
        : path.basename(document.uri.fsPath, path.extname(document.uri.fsPath));
      const output = await vscode.window.showSaveDialog({
        defaultUri: vscode.Uri.file(path.join(base, `${stem}.${extension}`)),
        filters: { [extension.toUpperCase()]: [extension] },
      });
      if (!output) return;
      if (output.scheme !== "file") throw new Error("首版只支持保存到本地文件");
      const catalog = await services.catalog(template);
      await services.fonts.refresh(services.directories());
      const missing = services.fonts
        .statuses(catalog)
        .filter((status) => !status.match);
      if (missing.length) {
        const choice = await vscode.window.showWarningMessage(
          `模板有 ${missing.length} 项字体缺失，可能使用系统回退字体。`,
          "字体管理",
          "继续导出",
        );
        if (choice === "字体管理") {
          await services.manageFonts(template);
          return;
        } else if (choice !== "继续导出") return;
      }
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: `导出 ${extension.toUpperCase()}`,
          cancellable: true,
        },
        async (progress, token) => {
          const controller = new AbortController();
          const subscription = token.onCancellationRequested(() =>
            controller.abort(),
          );
          let percent = 0;
          try {
            await services.exportSnapshot(
              text,
              base,
              extension as "pdf" | "png",
              output.fsPath,
              template,
              controller.signal,
              (event) => {
                const next = Math.max(percent, (event.fraction ?? 0) * 100);
                progress.report({
                  message: event.message ?? event.phase,
                  increment: next - percent,
                });
                percent = next;
              },
              settings,
            );
          } finally {
            subscription.dispose();
          }
        },
      );
      void vscode.window.showInformationMessage(`已导出 ${output.fsPath}`);
      try {
        await vscode.commands.executeCommand("revealFileInOS", output);
      } catch (error) {
        await vscode.window.showWarningMessage(
          `文件已导出，但无法在文件管理器中显示：${String(error)}`,
        );
      }
    });
  }
  const command = (name: string, action: () => Promise<unknown>) =>
    context.subscriptions.push(
      vscode.commands.registerCommand(name, async () => {
        try {
          await action();
        } catch (error) {
          if (!(error instanceof Error && error.name === "AbortError")) {
            services.output.appendLine(String(error));
            await vscode.window.showErrorMessage(String(error));
          }
        }
      }),
    );
  command("markview.export.pdf", () => exportDocument("pdf"));
  command("markview.export.png", () => exportDocument("png"));
  command("markview.export.template", () => exportDocument(undefined, true));
  command("markview.fonts", () => services.manageFonts());
  return services;
}
