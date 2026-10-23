import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { runInNewContext } from "node:vm";
const bundle = await build({
  entryPoints: ["vscode/src/preview/scroll-sync.ts"],
  bundle: true,
  platform: "node",
  format: "cjs",
  write: false,
  external: ["vscode"],
});

test("editor bridge suppresses only acknowledged reveals and accepts immediate reverse scrolls", () => {
  class Position {
    constructor(line, character = 0) {
      this.line = line;
      this.character = character;
    }
    isEqual(other) {
      return other.line === this.line && other.character === this.character;
    }
  }
  class Range {
    constructor(start, end) {
      this.start = start;
      this.end = end;
    }
  }
  const document = {
    uri: { toString: () => "file:///test.md" },
    version: 1,
    lineCount: 1000,
    lineAt: () => ({ text: "example text" }),
  };
  const messages = [],
    reveals = [];
  const selection = { untouched: true };
  const editor = {
    document,
    selection,
    visibleRanges: [new Range(new Position(0), new Position(19))],
    revealRange: (range) => reveals.push(range),
  };
  let visible;
  const vscode = {
    Position,
    Range,
    TextEditorRevealType: { AtTop: 1 },
    window: {
      visibleTextEditors: [editor],
      activeTextEditor: undefined,
      onDidChangeTextEditorSelection: () => ({ dispose() {} }),
      onDidChangeTextEditorVisibleRanges: (fn) => {
        visible = fn;
        return { dispose() {} };
      },
    },
  };
  const module = { exports: {} };
  runInNewContext(bundle.outputFiles[0].text, {
    module,
    exports: module.exports,
    require: () => vscode,
  });
  const bridge = new module.exports.EditorScrollSync(
    {
      webview: {
        postMessage: (message) => {
          messages.push(message);
          return Promise.resolve(true);
        },
      },
    },
    document,
    [],
    false,
  );
  const header = { uri: "file:///test.md", version: 1 };
  bridge.receive({ ...header, type: "rendered" });
  assert.equal(messages[0].line, 0);
  assert.equal(messages[0].maxLine, 980);
  const time = messages[0].inputTime + 10;
  bridge.receive({ ...header, type: "preview-control", inputTime: time });
  bridge.receive({
    ...header,
    type: "preview-scroll",
    inputTime: time,
    line: 300,
  });
  assert.equal(reveals.at(-1).start.line, 300);
  function scroll(top, end = top + 19) {
    editor.visibleRanges = [new Range(new Position(top), new Position(end))];
    visible({ textEditor: editor, visibleRanges: editor.visibleRanges });
  }
  bridge.receive({ ...header, type: "preview-hover", hovering: true });
  scroll(298);
  assert.equal(
    messages.length,
    1,
    "Partial visible lines must not steal preview control",
  );
  scroll(300);
  assert.equal(messages.length, 1, "Following a reveal must not echo back");
  scroll(299);
  assert.equal(
    messages.length,
    1,
    "Delayed reveal events must not interrupt a preview gesture",
  );
  bridge.receive({ ...header, type: "preview-hover", hovering: false });
  scroll(298);
  assert.equal(
    messages.at(-1).line,
    298,
    "A reverse gesture takes control without a timer",
  );
  bridge.receive({
    ...header,
    type: "preview-scroll",
    inputTime: time,
    line: 700,
  });
  assert.equal(
    reveals.length,
    1,
    "Obsolete preview input cannot override the editor",
  );
  const next = messages.at(-1).inputTime + 10;
  bridge.receive({
    ...header,
    type: "preview-scroll",
    inputTime: next,
    line: 1000,
  });
  assert.equal(reveals.at(-1).start.line, 980);
  const count = messages.length;
  scroll(980, 999);
  assert.equal(
    messages.length,
    count,
    "Bottom clamping must not create a feedback loop",
  );
  assert.equal(bridge.maxLine(), 980);
  scroll(979, 998);
  assert.equal(messages.at(-1).line, 979);
  document.version = 2;
  const previousReveals = reveals.length;
  bridge.receive({
    ...header,
    type: "preview-scroll",
    inputTime: next + 100,
    line: 900,
  });
  assert.equal(
    reveals.length,
    previousReveals,
    "Old document versions must be discarded",
  );
  assert.equal(editor.selection, selection);
});
