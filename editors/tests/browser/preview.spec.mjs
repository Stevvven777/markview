import {
  expect,
  test,
} from "../../../web/node_modules/@playwright/test/index.mjs";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { decodePng } from "../../../web/tests/png.mjs";

test.setTimeout(120_000);
const media = fileURLToPath(new URL("../../vscode/media/", import.meta.url));
const assets = fileURLToPath(
  new URL("../../../web/dist/assets/", import.meta.url),
);
const fonts = readdirSync(assets)
  .filter((file) => /\.(otf|ttf)$/.test(file))
  .map((file) => `/assets/${file}`);
const markdown =
  "# Adapter preview\n\n[External link](https://example.com)\n\n" +
  "Reading paragraph.\n\n".repeat(120);
const message = {
  type: "document",
  uri: "untitled:fixture",
  version: 1,
  markdown,
  fonts,
  options: {
    fontSize: 18,
    width: 760,
    justify: true,
    hyphenate: true,
    paragraphIndent: 0,
    theme: "light",
  },
  codeWrap: false,
  sync: true,
};
async function post(page, value) {
  await page.evaluate(
    (data) => window.dispatchEvent(new MessageEvent("message", { data })),
    value,
  );
}
async function ready(page, initial = message, follow) {
  await page.route("**/adapter.html", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: `<style>body { padding: 0 20px; }</style><link rel="stylesheet" href="/preview.css"><span id="status"></span><main id="reader"></main><script>window.messages=[];window.markviewHost={postMessage:message=>window.messages.push(message),setState:state=>window.savedPreview=state};</script><script type="module" src="/preview.js" data-wasm="/markview_web_bg.wasm"></script>`,
    }),
  );
  for (const file of ["preview.js", "preview.css"])
    await page.route(`**/${file}`, (route) =>
      route.fulfill({ path: `${media}${file}` }),
    );
  await page.goto("/adapter.html");
  await page.waitForFunction(() =>
    window.messages.some((item) => item.type === "ready"),
  );
  await post(page, initial);
  if (follow) await post(page, follow);
  await page.waitForFunction(
    () =>
      window.messages.some(
        (item) => item.type === "rendered" && item.version === 1,
      ),
    null,
    { timeout: 90_000 },
  );
  expect(
    await page.evaluate(() =>
      window.messages.filter((item) => item.type === "error"),
    ),
  ).toEqual([]);
}

test("production preview copies text, follows source, rejects stale requests and toggles scroll sync", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await ready(page);
  const canvas = page.locator("#reader canvas");
  await canvas.click();
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.press("ControlOrMeta+c");
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toContain("Adapter preview");
  await page.keyboard.press("Escape");
  const initial = await canvas.screenshot();
  const source = {
    type: "source",
    uri: message.uri,
    version: 1,
    offset: markdown.length - 300,
    inputTime: Date.now() + 1000,
    request: { documentVersion: 1, generation: 1 },
  };
  await post(page, source);
  await expect
    .poll(async () => initial.equals(await canvas.screenshot()))
    .toBe(false);
  const followed = await canvas.screenshot();
  await post(page, {
    ...source,
    version: 0,
    offset: 0,
    request: { documentVersion: 0, generation: 2 },
  });
  await page.waitForTimeout(150);
  expect(followed.equals(await canvas.screenshot())).toBe(true);
  const box = await canvas.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, -250);
  await page.waitForFunction(() =>
    window.messages.some((item) => item.type === "preview"),
  );
  await post(page, { ...message, sync: false });
  const count = await page.evaluate(
    () => window.messages.filter((item) => item.type === "preview").length,
  );
  await page.mouse.wheel(0, -250);
  await page.waitForTimeout(500);
  expect(
    await page.evaluate(
      () => window.messages.filter((item) => item.type === "preview").length,
    ),
  ).toBe(count);
});

test("scroll sync retains navigation during font loading and interpolates wrapped paragraphs, gaps and tall images", async ({
  page,
}) => {
  // Generate a tall image without adding a fixture or accessing the viewer internals.
  await page.goto("/");
  const image = await page.evaluate(() => {
    const canvas = document.createElement("canvas");
    canvas.width = 128;
    canvas.height = 1800;
    const context = canvas.getContext("2d");
    context.fillStyle = "red";
    context.fillRect(0, 0, 128, 900);
    context.fillStyle = "blue";
    context.fillRect(0, 900, 128, 900);
    return canvas.toDataURL();
  });
  const paragraph = "Wrapped 中文😀 e\u0301 **text** and words. ".repeat(300);
  const prefix = "# Long document\r\n\r\n";
  const gap = "\r\n".repeat(10);
  const imageSource = `![Tall image](${image})`;
  const source =
    prefix +
    paragraph +
    gap +
    imageSource +
    "\r\n\r\n" +
    "After the image.\r\n\r\n".repeat(120);
  const initialOffset = prefix.length + Math.floor(paragraph.length * 0.3);
  let generation = 1;
  const follow = (offset, version = 1) => ({
    type: "source",
    uri: message.uri,
    version,
    offset,
    inputTime: Date.now(),
    navigation: "scroll",
    request: {
      origin: "source",
      documentVersion: version,
      generation: generation++,
    },
  });
  await ready(page, { ...message, markdown: source }, follow(initialOffset));
  const position = () =>
    page.evaluate(
      () =>
        window.messages
          .filter((item) => ["position", "preview"].includes(item.type))
          .at(-1)?.offset,
    );
  await expect.poll(position).toBe(initialOffset);
  for (const offset of [
    prefix.length + Math.floor(paragraph.length * 0.75),
    prefix.length + paragraph.length + gap.length / 2,
  ]) {
    await post(page, follow(offset));
    await expect.poll(position).toBe(offset);
  }
  const imageStart = source.indexOf(imageSource);
  const canvas = page.locator("#reader canvas");
  const screenshots = [];
  for (const fraction of [0.15, 0.5, 0.8]) {
    const offset = imageStart + Math.floor(imageSource.length * fraction);
    await post(page, follow(offset));
    await expect.poll(position).toBe(offset);
    screenshots.push(await canvas.screenshot());
  }
  expect(screenshots[0].equals(screenshots[1])).toBe(false);
  expect(screenshots[1].equals(screenshots[2])).toBe(false);
  expect(
    await page.evaluate(() =>
      window.messages.filter((item) => item.type === "preview"),
    ),
  ).toEqual([]);

  const box = await canvas.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, -150);
  await page.waitForFunction(() =>
    window.messages.some((item) => item.type === "preview"),
  );
  await page.waitForTimeout(400);
  const user = await page.evaluate(() =>
    window.messages.filter((item) => item.type === "preview").at(-1),
  );
  expect(user.offset).toBeGreaterThan(imageStart);
  expect(user.offset).toBeLessThan(imageStart + imageSource.length);
  const beforeStale = await position();
  await post(page, { ...follow(0), inputTime: user.inputTime - 1 });
  await page.waitForTimeout(100);
  expect(await position()).toBe(beforeStale);

  await post(page, { ...follow(0), inputTime: user.inputTime + 1 });
  await expect.poll(position).toBe(0);
  await canvas.click();
  await page.keyboard.press("End");
  await expect.poll(position).toBeGreaterThan(imageStart + imageSource.length);
  const bottom = await position();
  await page.mouse.wheel(0, -10);
  await expect.poll(position).toBeLessThan(bottom);
  expect(bottom - (await position())).toBeLessThan(300);
});

test("touchpad pixel streams track distance without an animation tail and the persistent scrollbar supports dragging and keys", async ({
  page,
}) => {
  await ready(page);
  const bar = page.getByRole("scrollbar", { name: "预览滚动条" });
  await expect(bar).toBeVisible();
  const trace = [
    ...Array(64).fill(4.25),
    ...Array(16).fill(18.5),
    ...Array(16).fill(-7.25),
  ];
  const samples = await page.evaluate(async (deltas) => {
    const canvas = document.querySelector("#reader canvas");
    const bar = document.querySelector('[role="scrollbar"]');
    window.messages.length = 0;
    const samples = [];
    let total = 0;
    for (let i = 0; i < deltas.length; i += 4) {
      for (const deltaY of deltas.slice(i, i + 4)) {
        canvas.dispatchEvent(
          new WheelEvent("wheel", {
            deltaY,
            deltaMode: 0,
            bubbles: true,
            cancelable: true,
          }),
        );
        total += deltaY;
      }
      await new Promise(requestAnimationFrame);
      samples.push({
        expected: Math.round(total),
        actual: Number(bar.getAttribute("aria-valuenow")),
      });
    }
    return samples;
  }, trace);
  for (const sample of samples)
    expect(Math.abs(sample.actual - sample.expected)).toBeLessThanOrEqual(1);
  const last = samples.at(-1).actual;
  await page.waitForTimeout(250);
  await expect(bar).toHaveAttribute("aria-valuenow", String(last));
  const packets = await page.evaluate(() => ({
    controls: window.messages.filter((item) => item.type === "control").length,
    positions: window.messages.filter((item) =>
      ["position", "preview"].includes(item.type),
    ).length,
  }));
  expect(packets.controls).toBe(1);
  expect(packets.positions).toBeLessThanOrEqual(samples.length + 1);

  const thumb = page.locator(".preview-scrollbar-thumb");
  const thumbBox = await thumb.boundingBox(),
    track = await bar.boundingBox();
  await page.mouse.move(
    thumbBox.x + thumbBox.width / 2,
    thumbBox.y + thumbBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    track.x + track.width / 2,
    track.y + track.height * 0.8,
    { steps: 8 },
  );
  await page.mouse.up();
  await expect
    .poll(async () => Number(await bar.getAttribute("aria-valuenow")))
    .toBeGreaterThan(last + 500);
  await bar.press("End");
  await expect
    .poll(
      async () =>
        Number(await bar.getAttribute("aria-valuenow")) -
        Number(await bar.getAttribute("aria-valuemax")),
    )
    .toBe(0);
  await bar.press("Home");
  await expect(bar).toHaveAttribute("aria-valuenow", "0");
  await post(page, { ...message, version: 2, markdown: "Short document." });
  await expect(page.locator('[role="scrollbar"]')).toBeHidden();
});

test("production preview applies reading settings, code wrapping, themes", async ({
  page,
}) => {
  await ready(page);
  const canvas = page.locator("#reader canvas");
  const code =
    "# Settings\n\n```text\n" +
    "long_code_line ".repeat(80) +
    "\n```\n\n" +
    "Reading paragraph.\n\n".repeat(20);
  await post(page, { ...message, version: 2, markdown: code });
  await page.waitForFunction(() =>
    window.messages.some(
      (item) => item.type === "rendered" && item.version === 2,
    ),
  );
  const before = await canvas.screenshot();
  await post(page, {
    ...message,
    version: 3,
    markdown: code,
    codeWrap: true,
    options: {
      ...message.options,
      theme: "dark",
      fontSize: 24,
      width: 460,
      justify: false,
      hyphenate: false,
      paragraphIndent: 2,
    },
  });
  await page.waitForFunction(() =>
    window.messages.some(
      (item) => item.type === "rendered" && item.version === 3,
    ),
  );
  expect(before.equals(await canvas.screenshot())).toBe(false);
  expect(
    await page.evaluate(() =>
      window.messages.filter((item) => item.type === "error"),
    ),
  ).toEqual([]);
});

test("production preview decodes host image bytes and reports external links", async ({
  page,
}) => {
  await ready(page);
  await post(page, {
    ...message,
    version: 2,
    markdown:
      "[External link](https://example.com)\n\n![Local](images/red.png?cache=1)\n",
  });
  await page.waitForFunction(() =>
    window.messages.some(
      (item) => item.type === "resource" && item.version === 2,
    ),
  );
  const resource = await page.evaluate(() =>
    window.messages.find(
      (item) => item.type === "resource" && item.version === 2,
    ),
  );
  expect(resource.target).toBe("images/red.png?cache=1");
  const image =
    "iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAAlElEQVR4nO3QMREAMBDDsPAn/YWhoR60+7zb7mfTAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA3QAVoDdIDWAB2gNUAHaA9DiOHSbdjxEgAAAABJRU5ErkJggg==";
  await post(page, {
    type: "resource",
    uri: message.uri,
    version: 2,
    id: resource.id,
    bytes: image,
  });
  const canvas = page.locator("#reader canvas");
  await expect
    .poll(async () => {
      const png = decodePng(await canvas.screenshot());
      let red = 0;
      for (let i = 0; i < png.data.length; i += png.channels)
        if (png.data[i] > 200 && png.data[i + 1] < 30 && png.data[i + 2] < 30)
          red++;
      return red;
    })
    .toBeGreaterThan(1000);
  const box = await canvas.boundingBox();
  let point;
  for (let y = 4; y < 80 && !point; y += 8) {
    for (
      let x = (box.width - Math.min(box.width, 760)) / 2;
      x < box.width / 2;
      x += 24
    ) {
      await page.mouse.move(box.x + x, box.y + y);
      await page.evaluate(() => new Promise(requestAnimationFrame));
      if (
        await canvas.evaluate(
          (element) => getComputedStyle(element).cursor === "pointer",
        )
      ) {
        point = { x: box.x + x, y: box.y + y };
        break;
      }
    }
  }
  expect(point).toBeDefined();
  await page.mouse.click(point.x, point.y);
  await page.waitForFunction(() =>
    window.messages.some(
      (item) => item.type === "link" && item.target === "https://example.com",
    ),
  );
});

test("reading surface has responsive side margins and no toolbar", async ({
  page,
}) => {
  await ready(page);
  await expect(page.locator("nav")).toHaveCount(0);
  const canvas = page.locator("#reader canvas");
  await canvas.focus();
  await expect(canvas).toBeFocused();
  await expect(canvas).toHaveCSS("outline-style", "none");
  for (const width of [360, 640, 1280]) {
    await page.setViewportSize({ width, height: 720 });
    await expect
      .poll(async () => {
        const box = await page.locator("#reader canvas").boundingBox();
        return (
          box &&
          box.x >= 12 &&
          Math.abs(box.x - (width - box.x - box.width)) < 1 &&
          (await page.evaluate(
            () => document.body.scrollWidth <= window.innerWidth,
          ))
        );
      })
      .toBe(true);
  }
});

test("Van Gogh reading stylesheet changes the production preview", async ({
  page,
}) => {
  const stylesheet = readFileSync(
    new URL(
      "../../../crates/markview-core/styles/vangogh.mvss.toml",
      import.meta.url,
    ),
    "utf8",
  ).replace('targets = ["pdf"]', 'targets = ["ui", "pdf"]');
  await ready(page, {
    ...message,
    markdown:
      "# Van Gogh 梵高\n\n## 阅读样式\n\nBlue headings and golden rules. 中文排版预览。",
    stylesheet,
  });
  const png = decodePng(await page.locator("#reader canvas").screenshot());
  let gold = 0;
  for (let i = 0; i < png.data.length; i += png.channels)
    if (
      png.data[i] === 199 &&
      png.data[i + 1] === 157 &&
      png.data[i + 2] === 69
    )
      gold++;
  expect(gold).toBeGreaterThan(100);
  await page.screenshot({
    path: new URL(
      "../../../artifacts/vscode/vangogh-preview.png",
      import.meta.url,
    ).pathname,
  });
});

test("editing after a long code block follows only hidden input and yields to preview gestures", async ({
  page,
}) => {
  let text =
    "# Editing\n\n```js\n" + "const value = 1;\n".repeat(100) + "```\n\n";
  await ready(page, { ...message, markdown: text });
  const track = page.getByRole("scrollbar");
  const y = async () => Number(await track.getAttribute("aria-valuenow"));
  const max = async () => Number(await track.getAttribute("aria-valuemax"));
  await track.focus();
  await page.keyboard.press("End");
  await expect
    .poll(async () => Math.abs((await max()) - (await y())))
    .toBeLessThan(2);
  let version = 1;
  const edit = async (addition, sync = true) => {
    text += addition;
    version++;
    await post(page, {
      ...message,
      version,
      markdown: text,
      sync,
      edit: { version, offset: text.length, inputTime: Date.now() + 10 },
    });
    await page.waitForFunction(
      (v) =>
        window.messages.some((m) => m.type === "rendered" && m.version === v),
      version,
    );
  };
  const before = await y();
  await edit("New paragraph.\n\n".repeat(6));
  await expect.poll(y).toBeGreaterThan(before + 50);
  await expect
    .poll(async () => Math.abs((await max()) - (await y())))
    .toBeLessThan(3);
  // Typing on the already visible final line must not move the viewport.
  await edit("Visible");
  await page.waitForTimeout(150);
  const visible = await y();
  await edit(" text");
  await page.waitForTimeout(150);
  expect(Math.abs((await y()) - visible)).toBeLessThan(2);
  // A new gesture during correction cancels the outstanding animation.
  await edit("\n\nMore text.\n\n".repeat(10));
  const box = await page.locator("#reader canvas").boundingBox();
  await page.mouse.move(box.x + 100, box.y + 100);
  await page.mouse.wheel(0, -250);
  await page.waitForTimeout(60);
  const manual = await y();
  await page.waitForTimeout(200);
  expect(Math.abs((await y()) - manual)).toBeLessThan(2);
  expect((await max()) - (await y())).toBeGreaterThan(100);
  await edit("Sync disabled.\n\n".repeat(10), false);
  await page.waitForTimeout(150);
  expect(Math.abs((await y()) - manual)).toBeLessThan(3);
});

test("rapid edits and composition replacements retain only the latest version", async ({
  page,
}) => {
  const base =
    "# Continuous input\n\n```js\n" +
    'const longLine = "value";\n'.repeat(500) +
    "```\n\n";
  await ready(page, { ...message, markdown: base });
  const track = page.getByRole("scrollbar");
  await track.focus();
  await page.keyboard.press("End");
  const timings = await page.evaluate(
    async ({ message, base }) => {
      const original = window.markviewHost.postMessage;
      let done,
        started,
        version = 1;
      window.markviewHost.postMessage = (value) => {
        original(value);
        if (value.type === "rendered" && value.version === version)
          done?.(performance.now() - started);
      };
      const samples = { withoutFollow: [], withFollow: [] };
      for (const mode of Object.keys(samples)) {
        for (let i = 0; i < 12; i++) {
          version++;
          const markdown =
            base +
            "Input paragraph.\n\n".repeat(i + 1) +
            ["zhong", "中", "中文😀"][i % 3];
          const wait = new Promise((resolve) => {
            done = resolve;
          });
          started = performance.now();
          window.dispatchEvent(
            new MessageEvent("message", {
              data: {
                ...message,
                version,
                markdown,
                edit:
                  mode === "withFollow"
                    ? {
                        version,
                        offset: markdown.length,
                        inputTime: Date.now() + version,
                      }
                    : undefined,
              },
            }),
          );
          samples[mode].push(await wait);
        }
      }
      // Several updates announced in one turn must leave only the newest target.
      for (let i = 0; i < 20; i++) {
        version++;
        const markdown =
          base + "Newest paragraph.\n\n".repeat(25) + "中文".repeat(i + 1);
        window.dispatchEvent(
          new MessageEvent("message", {
            data: {
              ...message,
              version,
              markdown,
              edit: {
                version,
                offset: markdown.length,
                inputTime: Date.now() + version,
              },
            },
          }),
        );
      }
      return { samples, version };
    },
    { message, base },
  );
  await page.waitForFunction(
    (v) =>
      window.messages.some((m) => m.type === "rendered" && m.version === v),
    timings.version,
  );
  await expect
    .poll(
      async () =>
        Number(await track.getAttribute("aria-valuemax")) -
        Number(await track.getAttribute("aria-valuenow")),
    )
    .toBeLessThan(3);
  console.log(
    "Edit-to-render browser timings (ms):",
    JSON.stringify(timings.samples),
  );
});

test("input follows visual wraps inside a code line", async ({ page }) => {
  await page.setViewportSize({ width: 420, height: 400 });
  const prefix = "# Code wrapping\n\n```js\n" + "const value = 1;\n".repeat(40);
  let line = 'const result = "';
  const suffix = "\n```\n";
  await ready(page, {
    ...message,
    markdown: prefix + line + suffix,
    codeWrap: true,
  });
  const track = page.getByRole("scrollbar");
  await expect
    .poll(async () => Number(await track.getAttribute("aria-valuemax")))
    .toBeGreaterThan(100);
  await track.focus();
  await page.keyboard.press("End");
  await expect
    .poll(
      async () =>
        Number(await track.getAttribute("aria-valuemax")) -
        Number(await track.getAttribute("aria-valuenow")),
    )
    .toBeLessThan(2);
  const initial = Number(await track.getAttribute("aria-valuenow"));
  for (let i = 0; i < 8; i++) {
    line += "wrapped_code_text_".repeat(5);
    const version = i + 2;
    await post(page, {
      ...message,
      version,
      markdown: prefix + line + suffix,
      codeWrap: true,
      edit: {
        version,
        offset: prefix.length + line.length,
        inputTime: Date.now() + version,
      },
    });
    await page.waitForFunction(
      (v) =>
        window.messages.some((m) => m.type === "rendered" && m.version === v),
      version,
    );
  }
  expect(Number(await track.getAttribute("aria-valuenow"))).toBeGreaterThan(
    initial + 100,
  );
  await page.waitForTimeout(150);
  expect(
    Number(await track.getAttribute("aria-valuemax")) -
      Number(await track.getAttribute("aria-valuenow")),
  ).toBeLessThan(45);
});

test("preview persists its document and reading position for window restoration", async ({
  page,
}) => {
  await ready(page, { ...message, baseDirectory: "/tmp/fixture" });
  const track = page.getByRole("scrollbar");
  await expect
    .poll(async () => Number(await track.getAttribute("aria-valuemax")))
    .toBeGreaterThan(100);
  await track.focus();
  await page.keyboard.press("End");
  await expect
    .poll(() => page.evaluate(() => window.savedPreview?.offset ?? 0))
    .toBeGreaterThan(100);
  const saved = await page.evaluate(() => window.savedPreview);
  expect(saved.uri).toBe(message.uri);
  expect(saved.baseDirectory).toBe("/tmp/fixture");
  expect(saved.markdown).toBeUndefined();
});
