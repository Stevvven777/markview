import {
  expect,
  test,
} from "../../../web/node_modules/@playwright/test/index.mjs";
import { build } from "../../node_modules/esbuild/lib/main.js";
import { runInNewContext } from "node:vm";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../", import.meta.url));
const bundle = await build({
  entryPoints: [`${root}vscode/src/shared/font-panel.ts`],
  bundle: true,
  platform: "node",
  format: "cjs",
  external: ["vscode"],
  write: false,
});
const available = {
  definition: { id: "serif", lookfor: ["Example Serif"] },
  candidate: "Example Serif",
  match: { path: "/System/Fonts/Example.ttf" },
  source: "system",
  families: [],
};
const missing = {
  definition: { id: "sans-serif", lookfor: ["<Missing Font>"] },
  source: "system",
  families: [
    {
      id: "fixture",
      lookfor: ["Download Font"],
      sources: 1,
      license: "OFL-1.1",
    },
  ],
};

async function manager(page, extraRows = []) {
  const downloaded = new Set();
  const ids = [];
  let receive,
    close,
    configChanged,
    changed,
    directory = [],
    fail = true,
    controller,
    calls = 0;
  const panel = {
    webview: {
      cspSource: "'self'",
      asWebviewUri: (uri) => uri,
      onDidReceiveMessage: (fn) => {
        receive = fn;
        return { dispose() {} };
      },
      postMessage: (data) =>
        page.evaluate(
          (data) => window.dispatchEvent(new MessageEvent("message", { data })),
          data,
        ),
    },
    reveal() {},
    onDidDispose: (fn) => {
      close = fn;
      return { dispose() {} };
    },
    dispose() {
      close();
    },
  };
  const vscode = {
    Uri: { joinPath: (_base, file) => `/font-media/${file}` },
    ViewColumn: { Active: -1 },
    ConfigurationTarget: { Global: 1 },
    window: {
      createWebviewPanel: () => panel,
      showOpenDialog: async () => [{ fsPath: "/custom/fonts" }],
    },
    workspace: {
      onDidChangeConfiguration: (fn) => {
        configChanged = fn;
        return { dispose() {} };
      },
      getConfiguration: () => ({
        update: async (_key, value) => {
          directory = value;
          configChanged({ affectsConfiguration: () => true });
        },
      }),
    },
  };
  const services = {
    readingTemplate: () => ({ id: "default", name: "阅读字体" }),
    context: { extensionUri: "/extension" },
    changed: {
      event: (fn) => {
        changed = fn;
        return { dispose() {} };
      },
    },
    directories: () => directory,
    catalog: async () => ({}),
    fonts: {
      refresh: async () => {},
      statuses: () => [
        available,
        ...[missing, ...extraRows].map((row) =>
          downloaded.has(row.families[0]?.id)
            ? {
                ...row,
                match: { path: "/cache/download.ttf" },
                candidate: row.families[0].lookfor[0],
                source: "cache",
              }
            : row,
        ),
      ],
    },
    pickTemplate: async () => ({
      id: "custom",
      name: "Custom.mvss.toml",
      path: "/custom.mvss.toml",
    }),
    withTemplate: async (template, action) => action(template),
    downloadFamily: async (_template, destination, id, signal, progress) => {
      calls++;
      expect(destination).toBe("ui");
      ids.push(id);
      controller = signal;
      progress({ fraction: 0.4, message: "下载 40%" });
      await new Promise((resolve, reject) => {
        const timer = setTimeout(resolve, 100);
        signal.addEventListener(
          "abort",
          () => {
            clearTimeout(timer);
            reject(new Error("cancelled"));
          },
          { once: true },
        );
      });
      if (fail && id === "fixture") throw new Error("fixture network failure");
      downloaded.add(id);
      changed();
    },
  };
  const module = { exports: {} };
  const require = createRequire(import.meta.url);
  runInNewContext(bundle.outputFiles[0].text, {
    module,
    exports: module.exports,
    require: (name) => (name === "vscode" ? vscode : require(name)),
    AbortController,
  });
  const instance = new module.exports.FontPanel(services);
  await page.exposeFunction("fontHost", (message) => receive(message));
  await page.addInitScript(() => {
    window.acquireVsCodeApi = () => ({
      postMessage: (message) => window.fontHost(message),
    });
  });
  await page.route("**/font-theme.css", (route) =>
    route.fulfill({
      contentType: "text/css",
      body: ":root { --vscode-foreground:#1f1f1f; --vscode-editor-background:#ffffff; --vscode-font-family:system-ui; --vscode-font-size:13px; --vscode-panel-border:#e5e5e5; --vscode-descriptionForeground:#616161; --vscode-button-background:#0078d4; --vscode-button-foreground:#ffffff; --vscode-button-secondaryBackground:#e5e5e5; --vscode-button-secondaryForeground:#333333; --vscode-dropdown-background:#ffffff; --vscode-dropdown-foreground:#333333; --vscode-focusBorder:#0078d4; }",
    }),
  );
  await page.route("**/font-manager.html", (route) =>
    route.fulfill({ contentType: "text/html", body: panel.webview.html }),
  );
  for (const [name, file] of [
    ["fonts.js", "fonts.js"],
    ["fonts.css", "fonts.css"],
    ["brand.svg", "brand.svg"],
  ])
    await page.route(`**/font-media/${name}`, (route) =>
      route.fulfill({ path: `${root}vscode/media/${file}` }),
    );
  await page.goto("/font-manager.html");
  await expect(page.locator("article")).toHaveCount(2 + extraRows.length);
  return {
    instance,
    allowDownload: () => {
      fail = false;
    },
    directories: () => directory,
    calls: () => calls,
    signal: () => controller,
    ids: () => ids,
  };
}

test("font manager uses production host and UI for status, progress, retry, templates and directories", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const app = await manager(page);
  await expect(
    page.getByText("<Missing Font>", { exact: false }),
  ).toBeVisible();
  await expect(page.locator("article img")).toHaveCount(0);
  await expect(
    page.getByText("实际使用：Example Serif · 系统已安装"),
  ).toBeVisible();
  await page.getByRole("button", { name: "下载", exact: true }).click();
  await expect(page.locator("progress")).toHaveAttribute("value", "0.4");
  await expect(page.locator("#status")).toContainText("下载失败");
  app.allowDownload();
  await page.getByRole("button", { name: "下载", exact: true }).click();
  await expect(page.locator("#status")).toContainText("下载完成");
  await expect(
    page.getByText("实际使用：Download Font · 本地已下载（回退字体）"),
  ).toBeVisible();
  expect(app.calls()).toBe(2);
  await page.getByRole("button", { name: "导出模板字体", exact: true }).click();
  await page.getByRole("button", { name: "选择模板…", exact: true }).click();
  await expect(page.locator("#template")).toHaveText("Custom.mvss.toml");
  await page
    .getByRole("button", { name: "添加字体目录…", exact: true })
    .click();
  await expect.poll(app.directories).toEqual(["/custom/fonts"]);
  await page.setViewportSize({ width: 400, height: 800 });
  expect(
    await page.evaluate(() => document.body.scrollWidth <= innerWidth),
  ).toBe(true);
  expect(errors).toEqual([]);
  await page.screenshot({
    path: `${root}../artifacts/vscode/font-manager.png`,
    fullPage: true,
  });
  app.instance.dispose();
});

test("font manager cancels downloads and aborts them when closed", async ({
  page,
}) => {
  const app = await manager(page);
  await page.getByRole("button", { name: "下载", exact: true }).click();
  await page.getByRole("button", { name: "取消下载", exact: true }).click();
  await expect(page.locator("#status")).toHaveText("下载已取消");
  expect(app.signal().aborted).toBe(true);
  await page.getByRole("button", { name: "下载", exact: true }).click();
  await expect(page.locator("progress")).toBeVisible();
  app.instance.dispose();
  expect(app.signal().aborted).toBe(true);
});

test("bulk downloads deduplicate families, continue after failure and retry only missing fonts", async ({
  page,
}) => {
  const duplicate = {
    ...missing,
    definition: { id: "heading", lookfor: ["<Missing Font>"] },
  };
  const second = {
    ...missing,
    definition: { id: "mono", lookfor: ["Second"] },
    families: [{ id: "second", lookfor: ["Second"], sources: 1 }],
  };
  const manual = {
    ...missing,
    definition: { id: "manual", lookfor: ["Manual"] },
    families: [],
  };
  const app = await manager(page, [duplicate, second, manual]);
  const all = page.getByRole("button", {
    name: "下载全部缺失字体",
    exact: true,
  });
  await all.click();
  await expect(page.locator("#status")).toContainText("下载失败");
  await expect(page.locator("#status")).toContainText(
    "1 项缺失字体没有下载来源",
  );
  expect(app.ids()).toEqual(["fixture", "second"]);
  app.allowDownload();
  await all.click();
  await expect(page.locator("#status")).toContainText("下载完成：1");
  expect(app.ids()).toEqual(["fixture", "second", "fixture"]);
  await expect(all).toBeDisabled();
  app.instance.dispose();
});

test("cancelling a bulk download does not start the next family", async ({
  page,
}) => {
  const app = await manager(page, [
    {
      ...missing,
      definition: { id: "mono", lookfor: ["Second"] },
      families: [{ id: "second", lookfor: ["Second"], sources: 1 }],
    },
  ]);
  await page
    .getByRole("button", { name: "下载全部缺失字体", exact: true })
    .click();
  await page.getByRole("button", { name: "取消下载", exact: true }).click();
  await expect(page.locator("#status")).toHaveText("下载已取消");
  expect(app.ids()).toEqual(["fixture"]);
  app.instance.dispose();
});
