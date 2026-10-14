import type { FontPanelState } from "./font-panel";
declare function acquireVsCodeApi(): { postMessage(message: unknown): void };
const host = acquireVsCodeApi();
const send = (action: string, extra = {}) =>
  host.postMessage({ action, ...extra });
const status = document.querySelector<HTMLElement>("#status")!;
const progress = document.querySelector<HTMLProgressElement>("#progress")!;
let downloadable = false;
const list = document.querySelector<HTMLElement>("#fonts")!;
function busy(value: boolean) {
  document
    .querySelectorAll<HTMLButtonElement | HTMLSelectElement>("button, select")
    .forEach((control) => {
      control.disabled =
        (value && control.dataset.action !== "cancel") ||
        (control.dataset.action === "download-all" && !downloadable);
    });
  document.querySelector<HTMLButtonElement>('[data-action="cancel"]')!.hidden =
    !value;
  progress.hidden = !value;
}
function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text: string,
  className = "",
) {
  const node = document.createElement(tag);
  node.textContent = text;
  node.className = className;
  return node;
}
function render(state: FontPanelState) {
  downloadable = state.rows.some(
    (row) => !row.match && row.families.some((family) => family.sources > 0),
  );
  document.querySelector<HTMLElement>("#template")!.textContent =
    state.template.name;
  document.querySelector<HTMLButtonElement>(
    '[data-action="template"]',
  )!.hidden = state.destination !== "pdf";
  for (const tab of ["ui", "pdf"])
    document
      .querySelector(`[data-action="${tab}"]`)!
      .setAttribute("aria-pressed", String(tab === state.destination));
  list.replaceChildren(
    ...state.rows.map((row) => {
      const card = element("article", "");
      const heading = element("div", "", "heading");
      heading.append(
        element("h3", row.definition.id),
        element(
          "span",
          row.match ? "可用" : "缺失",
          row.match ? "available" : "missing",
        ),
      );
      card.append(
        heading,
        element("p", `指定字体：${row.definition.lookfor.join(" → ")}`),
      );
      if (row.match) {
        const source = {
          cache: "本地已下载",
          directory: "附加目录",
          system: "系统已安装",
        }[row.source];
        card.append(
          element(
            "p",
            `实际使用：${row.candidate} · ${source}${row.candidate !== row.definition.lookfor[0] ? "（回退字体）" : ""}`,
          ),
          element("p", row.match.path, "path"),
        );
      } else {
        const families = row.families.filter((family) => family.sources > 0);
        if (families.length) {
          const options = element("select", "");
          options.dataset.definition = row.definition.id;
          options.setAttribute("aria-label", `${row.definition.id} 下载字体`);
          for (const family of families) {
            const option = element(
              "option",
              `${family.lookfor[0]} · ${family.license ?? "模板提供"}`,
            );
            option.value = family.id;
            options.append(option);
          }
          const button = element("button", "下载", "primary");
          button.onclick = () => {
            busy(true);
            send("download", { id: row.definition.id, family: options.value });
          };
          card.append(options, button);
        } else
          card.append(
            element(
              "p",
              "模板未提供下载来源；可安装字体或添加本地字体目录。",
              "path",
            ),
          );
      }
      return card;
    }),
  );
  busy(state.busy);
  if (!state.busy)
    status.textContent = `${state.rows.length} 项字体 · ${state.rows.filter((row) => !row.match).length} 项缺失`;
}
document
  .querySelectorAll<HTMLButtonElement>("[data-action]")
  .forEach((button) => {
    button.onclick = () => {
      if (button.dataset.action === "download-all") {
        busy(true);
        send("download-all", {
          families: Object.fromEntries(
            [
              ...list.querySelectorAll<HTMLSelectElement>(
                "select[data-definition]",
              ),
            ].map((select) => [select.dataset.definition!, select.value]),
          ),
        });
      } else send(button.dataset.action!);
    };
  });
window.addEventListener("message", (event) => {
  const message = event.data;
  if (message.type === "state") render(message);
  if (message.type === "progress") {
    status.textContent = message.message;
    busy(message.busy);
    if (typeof message.fraction === "number") {
      progress.max = 1;
      progress.value = Math.max(0, Math.min(1, message.fraction));
    } else progress.removeAttribute("value");
  }
});
send("ready");
