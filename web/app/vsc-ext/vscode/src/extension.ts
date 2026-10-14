import * as vscode from "vscode";
import { FontPanel } from "./shared/font-panel";
import { Services } from "./shared/service";

export function activate(context: vscode.ExtensionContext) {
  const services = new Services(context);
  let panel: FontPanel | undefined;
  context.subscriptions.push(
    services,
    { dispose: () => panel?.dispose() },
    vscode.commands.registerCommand("markview.fonts", async () => {
      try {
        if (!panel || panel.disposed) panel = new FontPanel(services);
        await panel.show(undefined, "ui");
      } catch (error) {
        void vscode.window.showErrorMessage(`Markview: ${String(error)}`);
      }
    }),
  );
}
