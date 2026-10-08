import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { runInNewContext } from "node:vm";
import { test } from "node:test";

function event() {
  const listeners = new Set();
  return {
    subscribe: (listener) => {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    fire: (value) => Promise.all([...listeners].map((fn) => fn(value))),
  };
}
function harness() {
  class Position {
    constructor(line, character) {
      Object.assign(this, { line, character });
    }
    isEqual(other) {
      return this.line === other.line && this.character === other.character;
    }
  }
  class Range {
    constructor(start, end) {
      Object.assign(this, { start, end });
    }
  }
  const uri = (file) => ({
    scheme: "file",
    fsPath: file,
    path: file,
    toString: () => `file://${file}`,
  });
  const text = "# Scroll test\n\n" + "A long paragraph. ".repeat(80) + "\n\n";
  const lines = text.split("\n");
  const document = {
    uri: uri("/tmp/scroll.md"),
    version: 1,
    languageId: "markdown",
    getText: () => text,
    lineAt: (line) => ({
      text: lines[line],
      range: new Range(
        new Position(line, 0),
        new Position(line, lines[line].length),
      ),
    }),
    offsetAt: (point) =>
      lines.slice(0, point.line).reduce((n, line) => n + line.length + 1, 0) +
      point.character,
    positionAt: (offset) => {
      const prefix = text.slice(0, offset).split("\n");
      return new Position(prefix.length - 1, prefix.at(-1).length);
    },
  };
  const messages = [],
    reveals = [],
    commands = new Map();
  const receive = event(),
    visible = event(),
    selection = event(),
    change = event();
  const fontRequests = [];
  const fontManagement = [];
  let sync = true;
  const editor = {
    document,
    selection: new Range(new Position(0, 0), new Position(0, 0)),
    visibleRanges: [new Range(new Position(2, 300), new Position(3, 0))],
    revealRange: (range) => {
      reveals.push(range);
      editor.visibleRanges = [new Range(range.start, new Position(3, 0))];
      queueMicrotask(() =>
        visible.fire({
          textEditor: editor,
          visibleRanges: editor.visibleRanges,
        }),
      );
    },
  };
  const subscriptions = [];
  let serializer;
  const panel = {
    active: true,
    webview: {
      asWebviewUri: (value) => value,
      postMessage: async (value) => {
        messages.push(value);
        return true;
      },
      onDidReceiveMessage: receive.subscribe,
    },
    onDidChangeViewState: event().subscribe,
    onDidDispose: event().subscribe,
    dispose() {},
  };
  const configuration = {
    get: (key, fallback) => (key === "scrollSync" ? sync : fallback),
  };
  const vscode = {
    Uri: {
      file: uri,
      parse: (value) => uri(value.replace("file://", "")),
      joinPath: (base, ...parts) => uri([base.path, ...parts].join("/")),
    },
    Position,
    Range,
    ViewColumn: { Beside: 2 },
    TextEditorRevealType: { AtTop: 3 },
    ColorThemeKind: { Dark: 2, HighContrast: 3 },
    commands: {
      registerCommand: (id, callback) => {
        commands.set(id, callback);
        return { dispose() {} };
      },
    },
    extensions: {
      getExtension: () => ({
        activate: () => ({
          previewFonts: async (markdown) => {
            fontRequests.push(markdown);
            return [];
          },
          readingTemplate: () => ({ id: "default", name: "阅读字体" }),
          previewStylesheet: async () => undefined,
          manageFonts: async (template, destination) => {
            fontManagement.push({ template, destination });
          },
          output: { appendLine() {} },
          changed: { event: event().subscribe },
        }),
      }),
    },
    workspace: {
      openTextDocument: async () => document,
      getConfiguration: () => configuration,
      onDidChangeTextDocument: change.subscribe,
      onDidChangeConfiguration: event().subscribe,
      onWillSaveTextDocument: event().subscribe,
      onDidSaveTextDocument: event().subscribe,
    },
    window: {
      activeTextEditor: editor,
      visibleTextEditors: [editor],
      registerWebviewPanelSerializer: (_type, value) => {
        serializer = value;
        return { dispose() {} };
      },
      activeColorTheme: { kind: 1 },
      createWebviewPanel: () => panel,
      showErrorMessage: (message) => {
        throw new Error(message);
      },
      onDidChangeTextEditorVisibleRanges: visible.subscribe,
      onDidChangeTextEditorSelection: selection.subscribe,
      onDidChangeActiveColorTheme: event().subscribe,
    },
  };
  const module = { exports: {} };
  const require = createRequire(import.meta.url);
  runInNewContext(
    readFileSync(new URL("../dist/preview-test.cjs", import.meta.url), "utf8"),
    {
      exports: module.exports,
      module,
      require: (name) => (name === "vscode" ? vscode : require(name)),
      process,
      Buffer,
      URL,
      console,
      setTimeout,
      clearTimeout,
    },
  );
  const api = module.exports.activate(
    {
      subscriptions,
      extensionUri: uri("/extension"),
    },
    vscode.extensions.getExtension().activate(),
  );
  return {
    restore: (saved) => serializer.deserializeWebviewPanel(panel, saved),
    editor,
    fontRequests,
    fontManagement,
    edit: async (text, contentChanges) => {
      document.getText = () => text;
      document.version++;
      await change.fire({ document, contentChanges });
      await new Promise((resolve) => setTimeout(resolve, 10));
    },
    panel,
    document,
    messages,
    reveals,
    api,
    commands,
    setSync: (value) => {
      sync = value;
    },
    message: (value) =>
      receive.fire({
        uri: document.uri.toString(),
        version: document.version,
        ...value,
      }),
    scroll: async (position) => {
      editor.visibleRanges = [new Range(position, new Position(3, 0))];
      await visible.fire({
        textEditor: editor,
        visibleRanges: editor.visibleRanges,
      });
    },
    select: (position) =>
      selection.fire({
        textEditor: editor,
        selections: [{ start: position, isEmpty: false }],
        kind: 2,
      }),
  };
}

test("preview host follows the opening viewport and source scrolling while the preview has focus", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  await h.message({ type: "rendered", blocks: 3 });
  assert.equal(h.messages.at(-1).type, "source");
  assert.equal(
    h.messages.at(-1).offset,
    h.document.offsetAt(h.editor.visibleRanges[0].start),
  );
  assert.equal(h.panel.active, true);
  await h.scroll({ line: 2, character: 700 });
  assert.equal(
    h.messages.at(-1).offset,
    h.document.offsetAt({ line: 2, character: 700 }),
  );
  assert.equal(h.messages.at(-1).navigation, "scroll");
  assert.equal(h.reveals.length, 0);
  assert.equal(h.editor.selection.start.line, 0);
  h.setSync(false);
  const count = h.messages.length;
  await h.scroll({ line: 2, character: 800 });
  assert.equal(h.messages.length, count);
});

test("preview scrolling suppresses asynchronous editor feedback, preserves selection and permits a new source gesture", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  h.panel.active = false;
  const inputTime = Date.now() + 100;
  await h.message({ type: "control", inputTime });
  const count = h.messages.length;
  const offset = h.document.offsetAt({ line: 2, character: 900 });
  await h.message({ type: "preview", offset, inputTime });
  await new Promise(setImmediate);
  assert.equal(h.reveals.length, 1);
  assert.equal(h.messages.length, count);
  assert.equal(h.reveals[0].start.character, 900);
  assert.equal(h.reveals[0].end.character, h.document.lineAt(2).text.length);
  assert.equal(h.editor.selection.start.line, 0);
  await h.select({ line: 2, character: 50 });
  assert.equal(h.messages.at(-1).navigation, "selection");
  assert.ok(h.messages.at(-1).inputTime > inputTime);
  await h.message({ type: "preview", offset: 0, inputTime });
  assert.equal(h.reveals.length, 1);
  await new Promise((resolve) => setTimeout(resolve, 310));
  await h.scroll({ line: 2, character: 100 });
  assert.equal(
    h.messages.at(-1).offset,
    h.document.offsetAt({ line: 2, character: 100 }),
  );
});

test("the preview toolbar packages the original M-and-lines icon for both themes", () => {
  const manifest = JSON.parse(
    readFileSync(new URL("../vscode/package.json", import.meta.url)),
  );
  const command = manifest.contributes.commands.find(
    (item) => item.command === "markview.preview.open",
  );
  for (const file of Object.values(command.icon))
    assert.equal(
      readFileSync(new URL(`../vscode/${file}`, import.meta.url), "utf8"),
      readFileSync(
        new URL(`../../assets/ui/${file.split("/").at(-1)}`, import.meta.url),
        "utf8",
      ),
    );
});

test("rapid preview packets coalesce to the final position and source input cancels queued following", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  const start = Date.now();
  for (let i = 0; i < 100; i++)
    await h.message({ type: "preview", offset: 200 + i, inputTime: start + i });
  assert.ok(h.reveals.length <= 2);
  await new Promise((resolve) => setTimeout(resolve, 70));
  assert.ok(h.reveals.length <= 3);
  assert.equal(h.document.offsetAt(h.reveals.at(-1).start), 299);
  const count = h.reveals.length;
  await h.message({ type: "preview", offset: 400, inputTime: start + 100 });
  await h.select({ line: 2, character: 10 });
  await new Promise((resolve) => setTimeout(resolve, 70));
  assert.equal(h.reveals.length, count);
});

test("live edits load newly required emoji fonts without rescanning on ordinary typing", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  const before = h.fontRequests.length;
  await h.edit("Ordinary text 中文");
  assert.equal(h.fontRequests.length, before);
  await h.edit("Now with 😀");
  assert.equal(h.fontRequests.length, before + 1);
  assert.equal(h.fontRequests.at(-1), "Now with 😀");
  assert.ok(!h.panel.webview.html.includes("<nav"));
});

test("leaving the preview bottom keeps the same semantic mapping without an EOF jump", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  const inputTime = Date.now() + 1000;
  await h.message({ type: "control", inputTime });
  await h.message({ type: "preview", offset: 700, edge: "end", inputTime });
  assert.equal(h.document.offsetAt(h.reveals.at(-1).start), 700);
  await h.message({ type: "preview", offset: 695, inputTime });
  await new Promise((resolve) => setTimeout(resolve, 65));
  assert.equal(h.document.offsetAt(h.reveals.at(-1).start), 695);
});

test("reading font management opens the UI catalog without opening a preview", async () => {
  const h = harness();
  await h.commands.get("markview.preview.fonts")();
  assert.equal(h.fontManagement.length, 1);
  assert.equal(h.fontManagement[0].destination, "ui");
  assert.equal(h.fontManagement[0].template.id, "default");
  assert.equal(h.api.previews().length, 0);
});

test("content edits carry a versioned input target without a viewport-top follow", async () => {
  const h = harness();
  await h.commands.get("markview.preview.open")();
  await h.message({ type: "ready" });
  await h.message({ type: "rendered", blocks: 1 });
  const text = h.document.getText();
  await h.edit(text + "New paragraph", [
    { rangeOffset: text.length, rangeLength: 0, text: "New paragraph" },
  ]);
  const doc = h.messages.filter((m) => m.type === "document").at(-1);
  assert.equal(doc.edit.offset, text.length + 13);
  assert.equal(doc.edit.version, h.document.version);
  const count = h.messages.filter((m) => m.type === "source").length;
  await h.message({ type: "rendered", blocks: 2 });
  assert.equal(h.messages.filter((m) => m.type === "source").length, count);
  h.setSync(false);
  await h.edit(text + "Disabled", [
    { rangeOffset: text.length, rangeLength: 0, text: "Disabled" },
  ]);
  assert.equal(
    h.messages.filter((m) => m.type === "document").at(-1).edit,
    undefined,
  );
});

test("window restoration reconnects the existing panel and its document", async () => {
  const h = harness();
  await h.restore({
    uri: h.document.uri.toString(),
    baseDirectory: "/tmp",
    offset: 700,
  });
  assert.ok(h.panel.webview.html.includes("acquireVsCodeApi"));
  await h.message({ type: "ready" });
  assert.equal(
    h.messages.filter((m) => m.type === "document").at(-1).markdown,
    h.document.getText(),
  );
  await h.message({ type: "rendered", blocks: 2 });
  assert.equal(
    h.messages.filter((m) => m.type === "source").at(-1).offset,
    700,
  );
  await h.edit("Unsaved edit after reload");
  assert.equal(
    h.messages.filter((m) => m.type === "document").at(-1).markdown,
    "Unsaved edit after reload",
  );
});
