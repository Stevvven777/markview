import {
  Viewer,
  init,
  type FontSet,
  type MarkviewOptions,
  type ImageRequest,
} from "@markview/viewer";
import { FontLoader } from "@markview/fonts";
import { browserResources, decodeImage } from "@markview/resources";
import type { SyncRequest } from "@markview/scroll-sync";
import { PreviewScrollMap } from "./scroll-map";
import { PreviewScrollbar } from "./scrollbar";
declare global {
  interface Window {
    markviewHost: {
      postMessage(message: unknown): void;
      setState?(state: unknown): void;
    };
  }
}
interface DocumentMessage {
  type: string;
  uri: string;
  version: number;
  markdown: string;
  stylesheet?: string;
  edit?: { version: number; offset: number; inputTime: number };
  fonts: string[];
  baseUrl?: string;
  baseDirectory?: string;
  options: MarkviewOptions;
  codeWrap: boolean;
  sync: boolean;
  offset?: number;
  id?: number;
  bytes?: string;
  error?: string;
  request?: SyncRequest;
  inputTime?: number;
  navigation?: "scroll" | "selection" | "edit";
}
const host = window.markviewHost,
  status = document.querySelector<HTMLElement>("#status")!;
const wasmUrl =
  document.querySelector<HTMLScriptElement>("script[data-wasm]")!.dataset.wasm!;
let viewer: Viewer | undefined,
  scrollbar: PreviewScrollbar | undefined,
  fontSet: FontSet | undefined,
  current: DocumentMessage | undefined,
  announcedDocument: DocumentMessage | undefined,
  sourceTarget: DocumentMessage | undefined,
  scrollMap = new PreviewScrollMap(),
  fontsKey = "",
  generation = 0;
let editScroll: { from: number; to: number; started: number } | undefined;
let inputTime = 0,
  previewOwnsInput = false,
  followedGeneration = -1;
const takeControl = () => {
  inputTime = Math.max(Date.now(), inputTime + 1);
  sourceTarget = undefined;
  editScroll = undefined;
  if (!previewOwnsInput) send("control", { inputTime });
  previewOwnsInput = true;
};
const fontLoader = new FontLoader();
let updates = Promise.resolve(),
  resourceId = 0,
  renderedVersion = -1;
const resources = new Map<number, ImageRequest>();
const send = (type: string, extra = {}) =>
  host.postMessage({
    type,
    uri: current?.uri,
    version: current?.version,
    ...extra,
  });
const persist = (offset: number) =>
  host.setState?.({
    uri: current?.uri,
    baseDirectory: current?.baseDirectory,
    offset,
  });
const fail = (error: unknown) => {
  status.textContent = `加载失败：${String(error)}`;
  send("error", { target: String(error) });
};
window.addEventListener("message", (event) => {
  const message = event.data as DocumentMessage;
  if (message.type === "resource" && message.id !== undefined) {
    const request = resources.get(message.id);
    resources.delete(message.id);
    if (
      !request ||
      request.signal.aborted ||
      current?.uri !== message.uri ||
      current.version !== message.version
    )
      return;
    if (message.error) request.reject(message.error);
    else if (message.bytes)
      void decodeImage(
        Uint8Array.from(atob(message.bytes), (c) => c.charCodeAt(0)),
        request.signal,
      ).then(
        (pixels) => request.resolve(pixels),
        (error) => request.reject(String(error)),
      );
    return;
  }
  if (message.type === "source") {
    if (
      announcedDocument?.uri === message.uri &&
      announcedDocument.version === message.version &&
      message.request &&
      message.request.documentVersion === message.version &&
      message.request.generation > followedGeneration &&
      (message.inputTime ?? 0) >= inputTime
    ) {
      followedGeneration = message.request.generation;
      inputTime = message.inputTime ?? inputTime;
      previewOwnsInput = false;
      sourceTarget = message;
      editScroll = undefined;
    }
    return;
  }
  if (message.type === "document") {
    announcedDocument = message;
    sourceTarget = undefined;
    if (message.sync && message.edit && message.edit.inputTime >= inputTime) {
      inputTime = message.edit.inputTime;
      previewOwnsInput = false;
      sourceTarget = {
        ...message,
        offset: message.edit.offset,
        navigation: "edit",
      };
    } else editScroll = undefined;
    const announced = ++generation;
    updates = updates
      .then(() => (announced === generation ? update(message) : undefined))
      .catch((error) => {
        if (announced === generation) fail(error);
      });
  }
});
function followSource() {
  if (
    !viewer ||
    !sourceTarget ||
    sourceTarget.uri !== current?.uri ||
    sourceTarget.version !== current.version ||
    viewer.reader.markview.stats().pending
  )
    return;
  if (sourceTarget.navigation === "edit") {
    const geometry = viewer.sourceToPreview(sourceTarget.offset ?? 0);
    if (geometry) {
      const engine = viewer.reader.markview;
      const y = engine.scroll(),
        height = viewer.element.clientHeight;
      const { y: top, height: lineHeight } = geometry.rect;
      const bottom = top + lineHeight;
      const margin = Math.min(height / 4, Math.max(24, lineHeight * 2));
      const target =
        top < y
          ? top - margin
          : bottom > y + height
            ? bottom - height + margin
            : y;
      const to = Math.max(0, Math.min(engine.maxScroll(), target));
      if (Math.abs(to - y) > 1)
        editScroll = editScroll
          ? { ...editScroll, to }
          : { from: y, to, started: performance.now() };
    }
  } else if (sourceTarget.navigation === "selection")
    viewer.scrollToSource(sourceTarget.offset ?? 0);
  else
    viewer.scrollTo(
      scrollMap.get(viewer).map("source", sourceTarget.offset ?? 0),
    );
  sourceTarget = undefined;
}
async function update(message: DocumentMessage) {
  const started = performance.now();
  const ticket = generation;
  const key = JSON.stringify(message.fonts);
  let next: FontSet | undefined;
  if (key !== fontsKey || !fontSet) {
    [next] = await Promise.all([
      fontLoader.load({ sources: message.fonts }, { wasmUrl }),
      init({ wasmUrl }),
    ]);
  }
  if (ticket !== generation) {
    next?.destroy();
    return;
  }
  if (next) {
    const previous = fontSet;
    fontSet = next;
    fontsKey = key;
    if (viewer) viewer.setFonts(next);
    previous?.destroy();
  }
  const fontsReady = performance.now();
  const firstMount = !viewer;
  const previous = current;
  const remount =
    !!viewer &&
    (previous?.uri !== message.uri || previous.baseUrl !== message.baseUrl);
  if (remount) {
    scrollbar?.destroy();
    viewer?.destroy();
    viewer = undefined;
    resources.clear();
    renderedVersion = -1;
    followedGeneration = -1;
    scrollMap = new PreviewScrollMap();
  }
  current = message;
  persist(
    previous?.uri === message.uri
      ? (viewer?.readingPosition()?.offset ?? 0)
      : 0,
  );
  if (!viewer) {
    viewer = await Viewer.mount(
      document.querySelector<HTMLElement>("#reader")!,
      {
        markdown: message.markdown,
        fonts: fontSet,
        initialization: { wasmUrl },
        markview: message.options,
        resources: {
          onResources: (events) => {
            for (const event of events)
              if (event.kind === "request") {
                if (event.request.src.startsWith("data:")) {
                  browserResources({ onError: fail }).onResources?.([event]);
                  continue;
                }
                const id = ++resourceId;
                resources.set(id, event.request);
                event.request.signal.addEventListener(
                  "abort",
                  () => resources.delete(id),
                  { once: true },
                );
                send("resource", { id, target: event.request.src });
              }
          },
          onError: fail,
        },
        onError: fail,
        onStats: (stats) => {
          followSource();
          if (editScroll && viewer && !stats.pending) {
            const t = Math.min(
              1,
              (performance.now() - editScroll.started) / 100,
            );
            viewer.scrollTo(
              editScroll.from +
                (editScroll.to - editScroll.from) * (1 - (1 - t) ** 3),
            );
            if (t === 1) editScroll = undefined;
          }
          scrollbar?.update();
          if (
            !stats.pending &&
            current &&
            viewer?.getMarkdown() === current.markdown &&
            current.version !== renderedVersion
          ) {
            renderedVersion = current.version;
            send("rendered", {
              blocks: stats.blocks,
              target: JSON.stringify({
                parseMs: stats.parseMs,
                layoutMs: stats.layoutMs,
                frameMs: stats.frameMs,
                reused: stats.reused,
                backend: stats.backend,
                adapter: stats.adapter,
              }),
            });
          }
        },
        onUserInput: takeControl,
        onLink: (target) => send("link", { target }),
        onReadingPosition: (position) => {
          if (!viewer) return;
          const engine = viewer.reader.markview;
          const offset = Math.round(
            scrollMap.get(viewer).map("preview", engine.scroll()),
          );
          persist(offset);
          send(
            current?.sync && previewOwnsInput && position.reason !== "reflow"
              ? "preview"
              : "position",
            {
              offset,
              inputTime,
            },
          );
        },
      },
    );
    scrollbar = new PreviewScrollbar(viewer, (y) => {
      takeControl();
      viewer?.scrollTo(y);
    });
  } else {
    if (viewer.getMarkdown() !== message.markdown)
      viewer.setMarkdown(message.markdown);
    if (JSON.stringify(previous?.options) !== JSON.stringify(message.options))
      viewer.setOptions(message.options);
  }
  if (ticket !== generation) return;
  followSource();
  if (
    remount ||
    !previous ||
    previous.codeWrap !== message.codeWrap ||
    previous.stylesheet !== message.stylesheet ||
    previous.options.theme !== message.options.theme
  ) {
    viewer.registerStylesheet(
      "vscode-config",
      `format_version=2\nversion=1\n[[rule]]\nwhen=["code_block"]\nwrap=${message.codeWrap}`,
    );
    if (message.stylesheet)
      viewer.registerStylesheet("vscode-reader", message.stylesheet);
    viewer.setStylesheets([
      "vscode-config",
      ...(message.stylesheet ? ["vscode-reader"] : []),
      message.options.theme === "dark" ? "bundled:dark" : "bundled:light",
    ]);
  }
  if (firstMount)
    send("performance", {
      target: JSON.stringify({
        fontLoadAndWasmMs: fontsReady - started,
        mountAndConfigureMs: performance.now() - fontsReady,
      }),
    });
  status.textContent = message.fonts.length
    ? ""
    : "模板字体缺失，请打开字体管理";
}
window.addEventListener("pagehide", () => {
  generation++;
  resources.clear();
  scrollbar?.destroy();
  viewer?.destroy();
  fontSet?.destroy();
  fontLoader.clear();
});
host.postMessage({ type: "ready" });
