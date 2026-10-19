import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";
const root = process.cwd();
async function open(page, restoredOffset = 0) {
  await page.route("http://markview.test/**", async (route) => {
    const name = new URL(route.request().url()).pathname.slice(1);
    if (!name)
      return route.fulfill({
        contentType: "text/html",
        body: `<!doctype html><link rel="stylesheet" href="/preview.css"><span id="status"></span><main id="reader"></main><script>window.messages=[];window.markviewHost={postMessage:message=>messages.push(message),setState:state=>window.saved=state};</script><script type="module" src="/preview.js" data-wasm="/markview_web_bg.wasm"></script>`,
      });
    const file =
      name === "font.otf"
        ? path.resolve(
            root,
            "../../../crates/markview-layout/tests/fonts/NotoSerif-Regular-subset.otf",
          )
        : path.resolve(root, "vscode/media", name);
    await route.fulfill({
      body: await readFile(file),
      contentType: name.endsWith(".js")
        ? "text/javascript"
        : name.endsWith(".wasm")
          ? "application/wasm"
          : name.endsWith(".css")
            ? "text/css"
            : "application/octet-stream",
    });
  });
  await page.goto("http://markview.test/");
  await expect
    .poll(() =>
      page.evaluate(() => messages.some((message) => message.type === "ready")),
    )
    .toBe(true);
  const message = {
    type: "document",
    uri: "file:///test.md",
    version: 1,
    markdown: "# Preview\n\n" + "A paragraph of readable text.\n\n".repeat(60),
    fonts: ["http://markview.test/font.otf"],
    theme: "light",
    base: "/tmp",
    offset: restoredOffset,
  };
  await publish(page, message);
  return message;
}
async function publish(page, message) {
  await page.evaluate(
    (data) => window.dispatchEvent(new MessageEvent("message", { data })),
    message,
  );
}
async function rendered(page, version) {
  await expect
    .poll(() =>
      page.evaluate(
        (version) =>
          messages.some(
            (message) =>
              message.type === "rendered" && message.version === version,
          ),
        version,
      ),
    )
    .toBe(true);
  expect(
    await page.evaluate(() =>
      messages.filter((message) => message.type === "error"),
    ),
  ).toEqual([]);
}
test("real MVaaC renders edits, keeps balanced margins and supports scrollbar/theme updates", async ({
  page,
}) => {
  const message = await open(page);
  await rendered(page, 1);
  await expect(page.locator("canvas")).toBeVisible();
  const padding = await page.locator("#reader").evaluate((element) => {
    const style = getComputedStyle(element);
    return [style.paddingLeft, style.paddingRight];
  });
  expect(padding[0]).toEqual(padding[1]);
  await page.locator("canvas").focus();
  expect(
    await page
      .locator("canvas")
      .evaluate((element) => getComputedStyle(element).outlineStyle),
  ).toBe("none");
  await page.getByRole("scrollbar").focus();
  await page.keyboard.press("End");
  await expect.poll(() => page.evaluate(() => saved.offset)).toBeGreaterThan(0);
  await publish(page, {
    ...message,
    version: 2,
    markdown: "# Updated\n\nUnsaved changes",
    theme: "dark",
  });
  await rendered(page, 2);
  await expect(page.getByRole("scrollbar")).toBeHidden();
  await page.screenshot({ path: "test-results/preview-dark.png" });
});
test("latest update wins during initialization and saved reading position restores", async ({
  page,
}) => {
  const message = await open(page, 700);
  await rendered(page, 1);
  await expect.poll(() => page.evaluate(() => saved.offset)).toBeGreaterThan(0);
  for (let version = 2; version <= 12; version++)
    await publish(page, {
      ...message,
      version,
      markdown: `# Version ${version}\n\nLatest text.`,
    });
  await rendered(page, 12);
  expect(await page.evaluate(() => saved.uri)).toBe("file:///test.md");
});
