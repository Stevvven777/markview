import * as vscode from "vscode";
import { ScrollSync } from "../../../../../packages/scroll-sync/src/index";

interface Message {
  type: string;
  uri?: string;
  version?: number;
  inputTime?: number;
  line?: number;
  hovering?: boolean;
}

export class EditorScrollSync {
  private sync: ScrollSync;
  private inputTime = 0;
  private expected: number[] = [];
  private initial: boolean;
  private previewHovered = false;
  private editVersion?: number;
  private viewportLines = 0;
  private lastTop = "";

  constructor(
    private panel: vscode.WebviewPanel,
    private document: vscode.TextDocument,
    subscriptions: vscode.Disposable[],
    restored: boolean,
  ) {
    this.sync = new ScrollSync(document.version);
    this.initial = !restored;
    subscriptions.push(
      vscode.window.onDidChangeTextEditorVisibleRanges((event) => {
        if (
          event.textEditor.document !== document ||
          !event.visibleRanges.length
        )
          return;
        if (this.editVersion === document.version) return;
        const position = event.visibleRanges[0].start;
        const key = `${position.line}:${position.character}:${this.maxLine()}`;
        if (key === this.lastTop) return;
        this.lastTop = key;
        const top = position.line;
        const index = this.expected.indexOf(top);
        const bottom =
          event.visibleRanges.at(-1)!.end.line >= document.lineCount - 1;
        if (
          this.sync.owner === "preview" &&
          (this.previewHovered ||
            index >= 0 ||
            (bottom && this.expected.some((line) => line >= top)))
        ) {
          this.expected.splice(0, index < 0 ? this.expected.length : index + 1);
          return;
        }
        this.expected = [];
        this.follow();
      }),
      vscode.window.onDidChangeTextEditorSelection((event) => {
        if (event.textEditor.document !== document || event.kind === undefined)
          return;
        this.sync.takeControl("source");
        this.expected = [];
        this.inputTime = Math.max(Date.now(), this.inputTime + 1);
      }),
    );
  }

  edit(offset: number) {
    if (vscode.window.activeTextEditor?.document !== this.document) return;
    this.sync.setDocumentVersion(this.document.version);
    this.sync.takeControl("source");
    this.expected = [];
    this.initial = false;
    this.inputTime = Math.max(Date.now(), this.inputTime + 1);
    this.editVersion = this.document.version;
    return { offset, inputTime: this.inputTime, version: this.editVersion };
  }

  private editor() {
    return vscode.window.visibleTextEditors.find(
      (editor) => editor.document === this.document,
    );
  }

  maxLine(): number {
    const ranges = this.editor()?.visibleRanges;
    if (!ranges?.length) return this.document.lineCount - 1;
    const end = ranges.at(-1)!.end.line;
    const span = end - ranges[0].start.line;
    if (
      (this.sync.owner === "source" && end < this.document.lineCount - 1) ||
      this.viewportLines === 0
    )
      this.viewportLines = span;
    return Math.max(0, this.document.lineCount - 1 - this.viewportLines);
  }

  private follow() {
    const top = this.editor()?.visibleRanges[0]?.start;
    if (!top) return;
    this.sync.setDocumentVersion(this.document.version);
    this.sync.takeControl("source");
    this.inputTime = Math.max(Date.now(), this.inputTime + 1);
    const request = this.sync.begin("source");
    void this.panel.webview.postMessage({
      type: "source-scroll",
      uri: this.document.uri.toString(),
      version: this.document.version,
      line:
        top.line +
        top.character / (this.document.lineAt(top.line).text.length + 1),
      maxLine: this.maxLine(),
      request,
      inputTime: this.inputTime,
    });
  }

  receive(message: Message): boolean {
    if (
      ![
        "preview-hover",
        "preview-control",
        "preview-scroll",
        "rendered",
      ].includes(message.type)
    )
      return false;
    if (
      message.uri !== this.document.uri.toString() ||
      message.version !== this.document.version
    )
      return true;
    this.sync.setDocumentVersion(this.document.version);
    if (message.type === "preview-hover") {
      this.previewHovered = message.hovering === true;
      return true;
    }
    if (message.type === "rendered") {
      this.editVersion = undefined;
      if (this.initial) {
        this.initial = false;
        this.follow();
      }
      return true;
    }
    if (
      !Number.isFinite(message.inputTime) ||
      message.inputTime! < this.inputTime
    )
      return true;
    this.inputTime = message.inputTime!;
    this.editVersion = undefined;
    this.sync.takeControl("preview");
    if (message.type === "preview-control" || !Number.isFinite(message.line))
      return true;
    const editor = this.editor();
    if (!editor) return true;
    const line = Math.max(0, Math.min(this.maxLine(), message.line!));
    const row = Math.floor(line),
      text = this.document.lineAt(row).text;
    const position = new vscode.Position(
      row,
      Math.min(text.length, Math.round((line - row) * (text.length + 1))),
    );
    const top = editor.visibleRanges[0]?.start;
    if (top?.isEqual(position)) return true;
    this.expected.push(row);
    if (this.expected.length > 32) this.expected.shift();
    editor.revealRange(
      new vscode.Range(position, position),
      vscode.TextEditorRevealType.AtTop,
    );
    return true;
  }
}
