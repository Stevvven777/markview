import * as vscode from "vscode";
import path from "node:path";

export async function resourceDirectory(
  document: vscode.TextDocument,
  text: string,
): Promise<string | undefined> {
  if (!["file", "untitled"].includes(document.uri.scheme))
    throw new Error("首版只支持本地工作区的 Markdown 文档");
  if (
    document.uri.scheme === "file" ||
    (document.isUntitled && path.isAbsolute(document.uri.fsPath))
  )
    return path.dirname(document.uri.fsPath);
  const folders =
    vscode.workspace.workspaceFolders?.filter(
      (folder) => folder.uri.scheme === "file",
    ) ?? [];
  const targets = [
    ...text.matchAll(/!?\[[^\]]*\]\(\s*<?([^\s)>]+)/g),
    ...text.matchAll(/^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)/gm),
    ...text.matchAll(/<(?:img|a)\b[^>]*(?:src|href)\s*=\s*["']([^"']+)/gi),
  ];
  const relative = targets.some(
    (match) => !/^(?:[a-z][a-z\d+.-]*:|\/|#)/i.test(match[1]),
  );
  if (!relative) return folders[0]?.uri.fsPath ?? process.cwd();
  if (folders.length === 1) return folders[0].uri.fsPath;
  if (folders.length > 1)
    return (
      await vscode.window.showWorkspaceFolderPick({
        placeHolder: "选择图片等相对资源所在的工作区",
      })
    )?.uri.fsPath;
  return (
    await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      title: "选择相对资源基准目录",
    })
  )?.[0].fsPath;
}
