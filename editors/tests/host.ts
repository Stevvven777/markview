import * as assert from "node:assert/strict";
import * as vscode from "vscode";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { downloadFixture } from "./download-fixture";
import type { Services } from "../vscode/src/shared/service";

async function until(check: () => boolean, timeout = 45000) {
  const end = Date.now() + timeout;
  while (!check()) {
    if (Date.now() > end)
      throw new Error("Timed out waiting for rendered preview");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}
export async function run() {
  const exporter =
    vscode.extensions.getExtension<Services>("szdytom.markview")!;
  if (process.env.MARKVIEW_TEST_INSTALLED === "1")
    assert.ok(
      exporter.extensionUri.fsPath.includes(`${path.sep}extensions${path.sep}`),
    );

  const services = await vscode.extensions
    .getExtension<Services>("szdytom.markview")!
    .activate();
  assert.ok(
    (await vscode.commands.getCommands()).includes("markview.export.pdf"),
  );
  const fullFonts = await services.previewFonts();
  const plainFonts = await services.previewFonts("普通中文 and English");
  const emojiFonts = await services.previewFonts("Hello 😀");
  assert.deepEqual(emojiFonts, fullFonts);
  const bytes = async (files: string[]) =>
    (
      await Promise.all(files.map(async (file) => (await fs.stat(file)).size))
    ).reduce((sum, size) => sum + size, 0);
  console.log(
    `Preview font payload: full=${await bytes(fullFonts)}, plain=${await bytes(plainFonts)} bytes`,
  );
  if (process.env.MARKVIEW_PROFILE === "1") {
    const output = path.resolve(
      process.env.MARKVIEW_TEST_ROOT!,
      "../artifacts/vscode",
    );
    const source = await fs.readFile(
      path.join(output, "trial/试用.md"),
      "utf8",
    );
    await vscode.extensions
      .getExtension("vscode.markdown-language-features")!
      .activate();
    const samples = [];
    let html = "";
    for (let run = 0; run < 3; run++) {
      const started = performance.now();
      html = await vscode.commands.executeCommand<string>(
        "markdown.api.render",
        source,
      );
      samples.push(performance.now() - started);
    }
    await fs.writeFile(path.join(output, "native-preview-body.html"), html);
    await fs.writeFile(
      path.join(output, "native-preview-render.json"),
      JSON.stringify({ markdownToHtmlMs: samples }),
    );
    console.log("Native Markdown-to-HTML timing", samples);
  }
  await services.manageFonts({ id: "default", name: "阅读字体" }, "ui");
  assert.equal(
    vscode.window.tabGroups.activeTabGroup.activeTab?.label,
    "Markview 字体管理",
  );
  await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  const catalog = await services.catalog();
  assert.ok(catalog.templates.some((template) => template.id === "print"));
  const document = await vscode.workspace.openTextDocument({
    language: "markdown",
    content: "# Unsaved host document\n\nFirst paragraph.\n",
  });
  const editor = await vscode.window.showTextDocument(document);
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "markview-host-"));
  try {
    const output = path.join(dir, "unsaved.pdf");
    await services.exportSnapshot(document.getText(), dir, "pdf", output);
    assert.ok(
      (await fs.readFile(output)).subarray(0, 4).equals(Buffer.from("%PDF")),
    );
    const previous = Buffer.from("existing output");
    await fs.writeFile(output, previous);
    const cancelled = new AbortController();
    await assert.rejects(
      services.exportSnapshot(
        "Paragraph.\n\n".repeat(1000),
        dir,
        "png",
        output,
        undefined,
        cancelled.signal,
        () => cancelled.abort(),
      ),
      { name: "AbortError" },
    );
    assert.deepEqual(await fs.readFile(output), previous);
    assert.ok(
      !(await fs.readdir(dir)).some((file) =>
        file.startsWith(".markview-export-"),
      ),
    );
    const fixture = await downloadFixture(
      dir,
      path.join(
        process.env.MARKVIEW_TEST_ROOT!,
        "../crates/markview-core/tests/fonts/NotoSerif-Regular-subset.otf",
      ),
    );
    const template = { id: "custom", name: "Fixture", path: fixture.template };
    try {
      await assert.rejects(services.downloadFamily(template, "pdf", "fixture"));
      fixture.serve("valid");
      await services.downloadFamily(template, "pdf", "fixture");
      const catalog = await services.catalog(template);
      const status = services.fonts
        .statuses(catalog)
        .find((item) => item.definition.id === "serif")!;
      assert.equal(status.source, "cache");
      assert.deepEqual(await fs.readFile(status.match!.path), fixture.bytes);
      const metadata = JSON.parse(
        await fs.readFile(
          path.join(path.dirname(status.match!.path), "metadata.json"),
          "utf8",
        ),
      );
      assert.equal(metadata.license, "OFL-1.1");
      assert.equal(
        metadata.source_info[0].files[0].url,
        "http://1.1.1.1/fixture.otf",
      );
      await services.exportSnapshot(
        document.getText(),
        dir,
        "pdf",
        output,
        template,
      );
      assert.ok((await fs.readFile(output)).includes(Buffer.from("NotoSerif")));
      assert.ok((await services.previewFonts()).includes(status.match!.path));
      const downloadCancellation = new AbortController();
      fixture.serve("slow", downloadCancellation);
      await assert.rejects(
        services.downloadFamily(
          template,
          "pdf",
          "fixture",
          downloadCancellation.signal,
        ),
        { name: "AbortError" },
      );
      assert.deepEqual(await fs.readFile(status.match!.path), fixture.bytes);
      assert.ok(
        !(await fs.readdir(path.dirname(services.fonts.cache))).some((item) =>
          item.startsWith("download-"),
        ),
      );
    } finally {
      await fixture.dispose();
    }
    if (process.env.MARKVIEW_TEST_PREVIEW !== "0") {
      const extension = vscode.extensions.getExtension<{
        previews(): {
          uri: string;
          publishedVersion: number;
          blocks: number;
          phase: string;
          error: string;
          offset: number;
          active: boolean;
        }[];
      }>("szdytom.markview");
      assert.ok(extension);
      const preview = await extension.activate();
      const readerStyle = path.join(dir, "vangogh-reader.mvss.toml");
      await fs.writeFile(
        readerStyle,
        (
          await fs.readFile(
            path.resolve(
              process.env.MARKVIEW_TEST_ROOT!,
              "../crates/markview-core/styles/vangogh.mvss.toml",
            ),
            "utf8",
          )
        ).replace('targets = ["pdf"]', 'targets = ["ui", "pdf"]'),
      );
      await vscode.workspace
        .getConfiguration("markview")
        .update(
          "previewStyleFile",
          readerStyle,
          vscode.ConfigurationTarget.Global,
        );
      await vscode.commands.executeCommand("markview.preview.open");
      console.log("Preview opened", preview.previews());
      let previousPhase = "";
      await until(() => {
        const state = preview.previews()[0];
        if (state?.phase !== previousPhase) {
          previousPhase = state.phase;
          console.log("Preview phase", state);
        }
        if (state?.error) throw new Error(state.error);
        return state?.publishedVersion === document.version;
      });
      console.log(
        "Preview startup measured",
        JSON.stringify(preview.previews()[0]),
      );
      assert.ok(preview.previews()[0].blocks >= 2);
      await editor.edit((edit) =>
        edit.insert(new vscode.Position(0, 0), "# New unsaved heading\n\n"),
      );
      await until(() => {
        const state = preview.previews()[0];
        if (state?.error) throw new Error(state.error);
        return state?.publishedVersion === document.version;
      });
      console.log(
        "Preview update measured",
        JSON.stringify(preview.previews()[0]),
      );
      if (process.env.MARKVIEW_PROFILE === "1") {
        const trial = await fs.readFile(
          path.resolve(
            process.env.MARKVIEW_TEST_ROOT!,
            "../artifacts/vscode/trial/试用.md",
          ),
          "utf8",
        );
        await vscode.window.showTextDocument(document, vscode.ViewColumn.One);
        await editor.edit((edit) =>
          edit.replace(
            new vscode.Range(
              document.positionAt(0),
              document.positionAt(document.getText().length),
            ),
            trial,
          ),
        );
        await until(
          () => preview.previews()[0].publishedVersion === document.version,
        );
        for (let sample = 0; sample < 3; sample++) {
          await editor.edit((edit) =>
            edit.insert(new vscode.Position(2, 0), "编辑 "),
          );
          await until(
            () => preview.previews()[0].publishedVersion === document.version,
          );
          console.log(
            "Trial document edit measured",
            JSON.stringify(preview.previews()[0]),
          );
        }
        for (let sample = 0; sample < 20; sample++) {
          await editor.edit((edit) =>
            edit.insert(new vscode.Position(2, 0), "字"),
          );
          await new Promise((resolve) => setTimeout(resolve, 16));
        }
        await until(
          () => preview.previews()[0].publishedVersion === document.version,
        );
        console.log(
          "Trial burst measured",
          JSON.stringify(preview.previews()[0]),
        );
      }
      await fs.writeFile(
        path.join(dir, "local.png"),
        Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAAlElEQVR4nO3QMREAMBDDsPAn/YWhoR60+7zb7mfTAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA9DiOHSbdjxEgAAAABJRU5ErkJggg==",
          "base64",
        ),
      );
      const associated = vscode.Uri.file(path.join(dir, "saved.md")).with({
        scheme: "untitled",
      });
      const newDocument = await vscode.workspace.openTextDocument(associated);
      const newEditor = await vscode.window.showTextDocument(
        newDocument,
        vscode.ViewColumn.One,
      );
      await newEditor.edit((edit) =>
        edit.insert(
          new vscode.Position(0, 0),
          "# First save\n\nResource context.\n\n![local](local.png?cache=1)\n",
        ),
      );
      await vscode.commands.executeCommand("markview.preview.open");
      await vscode.window.showTextDocument(newDocument, vscode.ViewColumn.One);
      assert.ok(await newDocument.save());
      await until(() =>
        preview
          .previews()
          .some(
            (item) =>
              item.uri ===
              vscode.Uri.file(path.join(dir, "saved.md")).toString(),
          ),
      );
      const saved = await vscode.workspace.openTextDocument(
        vscode.Uri.file(path.join(dir, "saved.md")),
      );
      await vscode.window.showTextDocument(saved, vscode.ViewColumn.One);
      await vscode.commands.executeCommand("markview.preview.open");
      await until(() => {
        const item = preview
          .previews()
          .find((item) => item.uri === saved.uri.toString());
        if (item?.error) throw new Error(item.error);
        return item?.publishedVersion === saved.version;
      });
      await vscode.commands.executeCommand("markview.preview.close");
      const scrollDocument = await vscode.workspace.openTextDocument({
        language: "markdown",
        content:
          "# Scroll integration\n\n" +
          Array.from(
            { length: 400 },
            (_, i) => `Paragraph ${i}: 中文😀 reading content.`,
          ).join("\n\n"),
      });
      const scrollEditor = await vscode.window.showTextDocument(
        scrollDocument,
        vscode.ViewColumn.One,
      );
      const reveal = async (line: number) => {
        const point = new vscode.Position(line, 0);
        scrollEditor.revealRange(
          new vscode.Range(point, point),
          vscode.TextEditorRevealType.AtTop,
        );
        await until(() =>
          scrollEditor.visibleRanges.some(
            (range) => range.contains(point) && range.start.line > line - 35,
          ),
        );
        return scrollDocument.offsetAt(scrollEditor.visibleRanges[0].start);
      };
      const firstOffset = await reveal(100);
      const originalSelection = scrollEditor.selection;
      await vscode.commands.executeCommand("markview.preview.open");
      const scrollState = () =>
        preview
          .previews()
          .find((item) => item.uri === scrollDocument.uri.toString());
      await until(
        () => scrollState()?.publishedVersion === scrollDocument.version,
      );
      await until(
        () => Math.abs((scrollState()?.offset ?? 0) - firstOffset) <= 2,
      );
      // `revealRange` simulates source scrolling while focus stays in the preview.
      assert.equal(scrollState()?.active, true);
      const secondOffset = await reveal(300);
      await until(
        () => Math.abs((scrollState()?.offset ?? 0) - secondOffset) <= 2,
      );
      const reverseOffset = await reveal(120);
      await until(
        () => Math.abs((scrollState()?.offset ?? 0) - reverseOffset) <= 2,
      );
      assert.deepEqual(scrollEditor.selection, originalSelection);
      await vscode.commands.executeCommand("markview.preview.close");
      await vscode.window.showTextDocument(document, vscode.ViewColumn.One);
      await vscode.commands.executeCommand("markview.preview.open");
      await vscode.commands.executeCommand("markview.preview.close");
      await until(() => preview.previews().length === 0);
    }
    console.log(
      process.env.MARKVIEW_TEST_PREVIEW === "0"
        ? "Extension host integration passed: export and fonts without opening a preview."
        : "Extension host integration passed: export, unsaved content, preview updates, focus-independent scroll following and disposal.",
    );
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}
