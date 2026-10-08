import type * as vscode from "vscode";
import { activate as activateExport } from "./export";
import { activate as activatePreview } from "./preview/extension";

export function activate(context: vscode.ExtensionContext) {
  const services = activateExport(context);
  return Object.assign(services, activatePreview(context, services));
}
