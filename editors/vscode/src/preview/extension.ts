import { needsEmojiFont } from "../shared/preview-fonts";
import { resourceDirectory } from "../shared/resources";
import * as vscode from "vscode";
import path from "node:path";
import { randomBytes } from "node:crypto";
import {
  ScrollSync,
  type SyncRequest,
} from "../../../../web/packages/scroll-sync/src/index";
import type { Services } from "../shared/service";

interface Message {
  type: string;
  uri?: string;
  version?: number;
  offset?: number;
  target?: string;
  id?: number;
  blocks?: number;
  request?: SyncRequest;
  inputTime?: number;
  navigation?: "scroll" | "selection";
}
export function activate(context: vscode.ExtensionContext, services: Services) {
  const panels = new Map<
    string,
    {
      panel: vscode.WebviewPanel;
      document: vscode.TextDocument;
      sync: ScrollSync;
      publishedVersion: number;
      blocks: number;
      error: string;
      phase: string;
      timings: Record<string, number>;
      offset: number;
      update(): Promise<void>;
    }
  >();
  let activePreview: vscode.WebviewPanel | undefined;
  const run = (action: () => Promise<unknown>) => async () => {
    try {
      await action();
    } catch (error) {
      await vscode.window.showErrorMessage(`Markview：${String(error)}`);
    }
  };
  async function openPreview(
    document: vscode.TextDocument,
    restoredPanel?: vscode.WebviewPanel,
    restored?: { baseDirectory?: string; offset?: number },
  ) {
    const openedAt = Date.now();
    if (!document || document.languageId !== "markdown") return;
    const existing = panels.get(document.uri.toString());
    if (existing) {
      activePreview = existing.panel;
      existing.panel.reveal(vscode.ViewColumn.Beside);
      return;
    }
    const chosenBase =
      restored?.baseDirectory ??
      (await resourceDirectory(document, document.getText()));
    if (!chosenBase) return;
    let baseDirectory = chosenBase;
    const panel =
      restoredPanel ??
      vscode.window.createWebviewPanel(
        "markview.preview",
        `预览 · ${path.basename(document.uri.path)}`,
        vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: true },
      );
    panel.title = `预览 · ${path.basename(document.uri.path)}`;
    let restoredOffset = restored?.offset;
    activePreview = panel;
    const sync = new ScrollSync(document.version);
    const resources = new Set<AbortController>();
    let disposed = false,
      fontPaths: string[] | undefined,
      stylesheet: string | undefined,
      emojiFonts = false,
      updateGeneration = 0,
      inputTime = 0,
      followingUntil = 0;
    let previewTimer: ReturnType<typeof setTimeout> | undefined;
    let pendingPreview: Message | undefined;
    let lastReveal = 0;
    let revealedPoint: vscode.Position | undefined;
    let editStartedAt: number | undefined;
    let editTarget:
      { version: number; offset: number; inputTime: number } | undefined;
    let initialFollow = true;
    let savingUri: string | undefined;
    const state = {
      panel,
      document,
      sync,
      publishedVersion: -1,
      blocks: 0,
      error: "",
      phase: "created",
      timings: { panelMs: Date.now() - openedAt } as Record<string, number>,
      offset: 0,
      update,
    };
    panels.set(document.uri.toString(), state);
    const subscriptions: vscode.Disposable[] = [];
    const media = vscode.Uri.joinPath(context.extensionUri, "media");
    const uri = (file: string) =>
      panel.webview.asWebviewUri(vscode.Uri.joinPath(media, file)).toString();
    panel.webview.options = {
      enableScripts: true,
      localResourceRoots: [media],
    };
    const nonce = randomBytes(18).toString("base64");
    panel.webview.html = `<!doctype html><html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${panel.webview.cspSource} https: http: data: blob:; connect-src ${panel.webview.cspSource} https: http:; script-src 'nonce-${nonce}' 'wasm-unsafe-eval'; style-src ${panel.webview.cspSource} 'unsafe-inline';"><link rel="stylesheet" href="${uri("preview.css")}"></head><body><span id="status" role="status">正在加载…</span><main id="reader"></main><script nonce="${nonce}">window.markviewHost=acquireVsCodeApi();window.markviewHost.postMessage({type:"loading"});window.addEventListener('error',e=>window.markviewHost.postMessage({type:'error',target:e.message||'脚本加载失败'}));window.addEventListener('unhandledrejection',e=>window.markviewHost.postMessage({type:'error',target:String(e.reason)}));</script><script nonce="${nonce}" type="module" src="${uri("preview.js")}" data-wasm="${uri("markview_web_bg.wasm")}"></script></body></html>`;
    function editor() {
      return vscode.window.visibleTextEditors.find(
        (editor) => editor.document === state.document,
      );
    }
    async function update() {
      try {
        await publish();
      } catch (error) {
        state.error = String(error);
        services.output.appendLine(state.error);
        void vscode.window.showErrorMessage(`Markview：${state.error}`);
      }
    }
    async function publish() {
      const generation = ++updateGeneration;
      const doc = state.document;
      const text = doc.getText(),
        version = doc.version,
        identity = doc.uri.toString();
      const config = vscode.workspace.getConfiguration("markview");
      const fontStart = Date.now();
      const emoji = needsEmojiFont(text);
      if (!fontPaths || emoji !== emojiFonts) {
        const [paths, source] = await Promise.all([
          services.previewFonts(text),
          services.previewStylesheet(),
        ]);
        if (generation !== updateGeneration || disposed) return;
        fontPaths = paths;
        stylesheet = source;
        emojiFonts = emoji;
        state.timings.fontLookupMs = Date.now() - fontStart;
      }
      if (
        disposed ||
        generation !== updateGeneration ||
        state.document.uri.toString() !== identity ||
        state.document.version !== version
      )
        return;
      panel.webview.options = {
        enableScripts: true,
        localResourceRoots: [
          media,
          vscode.Uri.file(baseDirectory),
          ...fontPaths.map((file) => vscode.Uri.file(path.dirname(file))),
          ...(vscode.workspace.workspaceFolders?.map((folder) => folder.uri) ??
            []),
          ...(doc.uri.scheme === "file"
            ? [vscode.Uri.file(path.dirname(doc.uri.fsPath))]
            : []),
        ],
      };
      sync.setDocumentVersion(version);
      const sourceDirectory = baseDirectory;
      const baseUrl = sourceDirectory
        ? panel.webview
            .asWebviewUri(vscode.Uri.file(sourceDirectory + path.sep))
            .toString()
        : undefined;
      await panel.webview.postMessage({
        type: "document",
        uri: identity,
        version,
        markdown: text,
        edit: editTarget?.version === version ? editTarget : undefined,
        stylesheet,
        fonts: fontPaths.map((file) =>
          panel.webview.asWebviewUri(vscode.Uri.file(file)).toString(),
        ),
        baseUrl,
        baseDirectory,
        options: {
          theme:
            vscode.window.activeColorTheme.kind ===
              vscode.ColorThemeKind.Dark ||
            vscode.window.activeColorTheme.kind ===
              vscode.ColorThemeKind.HighContrast
              ? "dark"
              : "light",
          fontSize: config.get("fontSize", 18),
          width: config.get("width", 760),
          justify: config.get("justify", true),
          hyphenate: config.get("hyphenate", true),
          paragraphIndent: config.get("paragraphIndent", 0),
        },
        codeWrap: config.get("codeWrap", false),
        sync: config.get("scrollSync", true),
      });
    }
    async function sourceFollow(
      offset: number,
      navigation: "scroll" | "selection" = "scroll",
    ) {
      clearTimeout(previewTimer);
      previewTimer = undefined;
      pendingPreview = undefined;
      editTarget = undefined;
      revealedPoint = undefined;
      inputTime = Math.max(Date.now(), inputTime + 1);
      sync.setDocumentVersion(state.document.version);
      sync.takeControl("source");
      const request = sync.begin("source");
      if (request && sync.isCurrent(request))
        await panel.webview.postMessage({
          type: "source",
          uri: state.document.uri.toString(),
          version: state.document.version,
          offset,
          request,
          inputTime,
          navigation,
        });
    }
    function revealPreview() {
      previewTimer = undefined;
      const message = pendingPreview;
      pendingPreview = undefined;
      const source = editor();
      if (
        !message ||
        !source ||
        disposed ||
        message.uri !== state.document.uri.toString() ||
        message.version !== state.document.version ||
        message.inputTime !== inputTime ||
        sync.owner !== "preview" ||
        !vscode.workspace.getConfiguration("markview").get("scrollSync", true)
      )
        return;
      const point = state.document.positionAt(message.offset ?? 0);
      if (revealedPoint?.isEqual(point)) return;
      revealedPoint = point;
      lastReveal = Date.now();
      // Filter the asynchronous editor scroll caused by this reveal.
      followingUntil = lastReveal + 100;
      source.revealRange(
        new vscode.Range(point, state.document.lineAt(point.line).range.end),
        vscode.TextEditorRevealType.AtTop,
      );
    }
    subscriptions.push(
      panel.webview.onDidReceiveMessage(async (message: Message) => {
        try {
          if (message.type === "loading") {
            state.phase = "loading";
            return;
          }
          if (message.type === "ready") {
            state.phase = "ready";
            state.timings.webviewReadyMs = Date.now() - openedAt;
            await update();
            return;
          }
          if (message.type === "performance") {
            services.output.appendLine(`[preview stages] ${message.target}`);
            return;
          }
          if (message.type === "settings") {
            await vscode.commands.executeCommand(
              "workbench.action.openSettings",
              "markview",
            );
            return;
          }
          if (message.type === "sync") {
            const config = vscode.workspace.getConfiguration("markview");
            await config.update(
              "scrollSync",
              !config.get("scrollSync", true),
              vscode.ConfigurationTarget.Global,
            );
            return;
          }
          if (message.type === "fonts") {
            await services.manageFonts(
              { id: "default", name: "阅读字体" },
              "ui",
            );
            return;
          }
          if (message.type === "close") {
            panel.dispose();
            return;
          }
          if (message.type === "error") {
            state.error = message.target ?? "预览失败";
            throw new Error(state.error);
          }
          if (
            message.uri !== state.document.uri.toString() ||
            message.version !== state.document.version
          )
            return;
          if (message.type === "control") {
            if ((message.inputTime ?? 0) >= inputTime) {
              inputTime = message.inputTime!;
              sync.takeControl("preview");
              editTarget = undefined;
            }
          } else if (message.type === "rendered") {
            if (state.timings.firstRenderMs === undefined) {
              state.timings.firstRenderMs = Date.now() - openedAt;
              services.output.appendLine(
                `[preview startup] ${JSON.stringify(state.timings)}`,
              );
            }
            if (editStartedAt !== undefined) {
              state.timings.lastEditToRenderMs = Date.now() - editStartedAt;
              if (state.timings.lastEditToRenderMs >= 100)
                services.output.appendLine(
                  `[preview slow update] ${state.timings.lastEditToRenderMs} ms ${message.target ?? ""}`,
                );
              editStartedAt = undefined;
            }
            state.publishedVersion = message.version!;
            state.blocks = message.blocks ?? 0;
            const edited = editTarget?.version === message.version;
            if (edited) editTarget = undefined;
            if (initialFollow && restoredOffset !== undefined) {
              const offset = restoredOffset;
              restoredOffset = undefined;
              initialFollow = false;
              await sourceFollow(
                Math.min(state.document.getText().length, Math.max(0, offset)),
              );
            } else if (!edited && (initialFollow || sync.owner === "source")) {
              const follow = !initialFollow || inputTime === 0;
              initialFollow = false;
              const top = editor()?.visibleRanges[0]?.start;
              if (
                top &&
                follow &&
                vscode.workspace
                  .getConfiguration("markview")
                  .get("scrollSync", true)
              )
                await sourceFollow(state.document.offsetAt(top));
            }
          } else if (message.type === "position") {
            state.offset = message.offset ?? 0;
          } else if (
            message.type === "resource" &&
            message.target &&
            message.id !== undefined
          ) {
            const controller = new AbortController();
            resources.add(controller);
            const identity = state.document.uri.toString(),
              version = state.document.version;
            try {
              const source = message.target;
              let bytes: Uint8Array;
              if (/^https?:/i.test(source)) {
                const response = await fetch(source, {
                  signal: controller.signal,
                });
                if (!response.ok)
                  throw new Error(`图片请求失败 ${response.status}`);
                const length = Number(response.headers.get("content-length"));
                if (length > 32 * 1024 * 1024)
                  throw new Error("图片超过 32 MiB");
                bytes = new Uint8Array(await response.arrayBuffer());
              } else {
                const file = path.isAbsolute(source)
                  ? source
                  : vscode.Uri.parse(
                      new URL(
                        source,
                        vscode.Uri.file(baseDirectory + path.sep).toString(),
                      ).href,
                    ).fsPath;
                bytes = await vscode.workspace.fs.readFile(
                  vscode.Uri.file(file),
                );
              }
              if (bytes.length > 32 * 1024 * 1024)
                throw new Error("图片超过 32 MiB");
              if (
                !disposed &&
                !controller.signal.aborted &&
                state.document.version === version
              )
                await panel.webview.postMessage({
                  type: "resource",
                  uri: identity,
                  version,
                  id: message.id,
                  bytes: Buffer.from(bytes).toString("base64"),
                });
            } catch (error) {
              if (!disposed)
                await panel.webview.postMessage({
                  type: "resource",
                  uri: identity,
                  version,
                  id: message.id,
                  error: String(error),
                });
            } finally {
              resources.delete(controller);
            }
          } else if (
            message.type === "preview" &&
            vscode.workspace
              .getConfiguration("markview")
              .get("scrollSync", true)
          ) {
            if ((message.inputTime ?? 0) < inputTime) return;
            inputTime = message.inputTime ?? inputTime;
            sync.takeControl("preview");
            editTarget = undefined;
            state.offset = message.offset ?? 0;
            pendingPreview = message;
            if (!previewTimer) {
              const remaining = 50 - (Date.now() - lastReveal);
              if (remaining <= 0) revealPreview();
              else previewTimer = setTimeout(revealPreview, remaining);
            }
          } else if (message.type === "link" && message.target) {
            const target = message.target;
            if (target.startsWith("#")) return;
            const base = vscode.Uri.file(baseDirectory + path.sep);
            const resolved = /^[a-z][a-z\d+.-]*:/i.test(target)
              ? vscode.Uri.parse(target)
              : vscode.Uri.parse(new URL(target, base.toString()).href);
            if (resolved) {
              if (resolved.scheme === "file")
                await vscode.commands.executeCommand("vscode.open", resolved);
              else if (["http", "https", "mailto"].includes(resolved.scheme))
                await vscode.env.openExternal(resolved);
            }
          }
        } catch (error) {
          state.error = String(error);
          services.output.appendLine(state.error);
          void vscode.window.showErrorMessage(`Markview：${state.error}`);
        }
      }),
      vscode.workspace.onDidChangeTextDocument((event) => {
        if (event.document === state.document) {
          editStartedAt = Date.now();
          const source = vscode.window.activeTextEditor;
          if (
            source?.document === state.document &&
            event.contentChanges?.length &&
            vscode.workspace
              .getConfiguration("markview")
              .get("scrollSync", true)
          ) {
            clearTimeout(previewTimer);
            pendingPreview = undefined;
            followingUntil = 0;
            inputTime = Math.max(Date.now(), inputTime + 1);
            sync.takeControl("source");
            // Changes use the old document coordinates; translate their ends once.
            let delta = 0;
            const ends = [...event.contentChanges]
              .sort((a, b) => a.rangeOffset - b.rangeOffset)
              .map((change) => {
                const end = change.rangeOffset + delta + change.text.length;
                delta += change.text.length - change.rangeLength;
                return end;
              });
            const caret = state.document.offsetAt(
              source.selection.active ?? source.selection.end,
            );
            const offset = ends.reduce((best, end) =>
              Math.abs(end - caret) < Math.abs(best - caret) ? end : best,
            );
            editTarget = {
              version: state.document.version,
              offset,
              inputTime,
            };
          } else editTarget = undefined;
          resources.forEach((controller) => controller.abort());
          void update().catch((error) =>
            vscode.window.showErrorMessage(String(error)),
          );
        }
      }),
      vscode.window.onDidChangeTextEditorVisibleRanges((event) => {
        if (
          event.textEditor.document === state.document &&
          Date.now() >= followingUntil &&
          editTarget?.version !== state.document.version &&
          event.visibleRanges.length > 0 &&
          vscode.workspace.getConfiguration("markview").get("scrollSync", true)
        )
          void sourceFollow(
            state.document.offsetAt(
              event.visibleRanges[0]?.start ?? new vscode.Position(0, 0),
            ),
          );
      }),
      vscode.window.onDidChangeTextEditorSelection((event) => {
        if (
          event.textEditor.document === state.document &&
          !event.selections[0].isEmpty &&
          event.kind !== undefined
        ) {
          followingUntil = 0;
          void sourceFollow(
            state.document.offsetAt(event.selections[0].start),
            "selection",
          );
        }
      }),
      vscode.window.onDidChangeActiveColorTheme(() => void update()),
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("markview")) {
          fontPaths = undefined;
          void update();
        }
      }),
      services.changed.event(() => {
        fontPaths = undefined;
        void update();
      }),
      panel.onDidChangeViewState((event) => {
        if (panel.active) activePreview = panel;
        if (event.webviewPanel.visible) void update();
      }),
      vscode.workspace.onWillSaveTextDocument((event) => {
        if (
          state.document.isUntitled &&
          vscode.window.activeTextEditor?.document === state.document
        )
          savingUri = event.document.uri.toString();
      }),
      vscode.workspace.onDidSaveTextDocument((saved) => {
        const associatedPath =
          state.document.isUntitled &&
          path.isAbsolute(state.document.uri.fsPath)
            ? vscode.Uri.file(state.document.uri.fsPath).toString()
            : undefined;
        if (
          state.document.isUntitled &&
          !saved.isUntitled &&
          (saved.uri.toString() === savingUri ||
            saved.uri.toString() === associatedPath)
        ) {
          savingUri = undefined;
          const existing = panels.get(saved.uri.toString());
          if (existing && existing !== state) existing.panel.dispose();
          panels.delete(state.document.uri.toString());
          state.document = saved;
          baseDirectory = path.dirname(saved.uri.fsPath);
          panels.set(saved.uri.toString(), state);
          panel.title = `预览 · ${path.basename(saved.uri.path)}`;
          void update();
        }
      }),
      panel.onDidDispose(() => {
        disposed = true;
        clearTimeout(previewTimer);
        if (activePreview === panel) activePreview = undefined;
        resources.forEach((controller) => controller.abort());
        updateGeneration++;
        sync.cancel();
        panels.delete(state.document.uri.toString());
        subscriptions.forEach((subscription) => subscription.dispose());
      }),
    );
  }
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "markview.preview.open",
      run(async () => {
        const document = vscode.window.activeTextEditor?.document;
        if (document) await openPreview(document);
      }),
    ),
    vscode.window.registerWebviewPanelSerializer("markview.preview", {
      async deserializeWebviewPanel(panel, raw) {
        const saved = raw as
          | { uri?: unknown; baseDirectory?: unknown; offset?: unknown }
          | undefined;
        try {
          if (!saved || typeof saved.uri !== "string")
            throw new Error("预览没有可恢复的文档地址，请关闭并重新打开一次。");
          const document = await vscode.workspace.openTextDocument(
            vscode.Uri.parse(saved.uri),
          );
          await openPreview(document, panel, {
            baseDirectory:
              typeof saved.baseDirectory === "string"
                ? saved.baseDirectory
                : undefined,
            offset:
              typeof saved.offset === "number" && Number.isFinite(saved.offset)
                ? saved.offset
                : 0,
          });
        } catch (error) {
          panel.webview.html =
            "<!doctype html><html><body>无法恢复预览。请确认源文件仍存在，然后重新打开预览。</body></html>";
          void vscode.window.showErrorMessage(`Markview：${String(error)}`);
        }
      },
    }),
    vscode.commands.registerCommand("markview.preview.close", () => {
      activePreview?.dispose();
    }),
    vscode.commands.registerCommand(
      "markview.preview.fonts",
      run(async () => {
        await services.manageFonts(services.readingTemplate(), "ui");
      }),
    ),
    vscode.commands.registerCommand("markview.preview.settings", () =>
      vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "markview",
      ),
    ),
    vscode.commands.registerCommand("markview.preview.sync", async () => {
      const config = vscode.workspace.getConfiguration("markview");
      await config.update(
        "scrollSync",
        !config.get("scrollSync", true),
        vscode.ConfigurationTarget.Global,
      );
    }),
    {
      dispose: () => {
        for (const state of panels.values()) state.panel.dispose();
        panels.clear();
      },
    },
  );
  return {
    previews: () =>
      [...panels.values()].map((state) => ({
        uri: state.document.uri.toString(),
        publishedVersion: state.publishedVersion,
        blocks: state.blocks,
        phase: state.phase,
        timings: state.timings,
        offset: state.offset,
        active: state.panel.active,
        error: state.error,
      })),
  };
}
