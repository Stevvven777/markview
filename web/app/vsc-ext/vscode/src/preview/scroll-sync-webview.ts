import type { Viewer, ReadingPosition } from "@markview/viewer";
import { ScrollMap, type SyncRequest } from "@markview/scroll-sync";

export interface EditPosition {
  offset: number;
  inputTime: number;
  version: number;
}

export interface SourceScroll {
  type: "source-scroll";
  uri: string;
  version: number;
  line: number;
  maxLine: number;
  inputTime: number;
  request: SyncRequest;
}

export class PreviewScrollSync {
  private starts: number[] = [];
  private maxLine = 0;
  private revision = -1;
  private maxScroll = -1;
  private map?: ScrollMap;
  private viewer?: Viewer;
  private pending?: SourceScroll;
  private inputTime = 0;
  private generation = -1;
  private ownsInput = false;
  private version = -1;
  private hovering?: boolean;
  private edit?: EditPosition;
  private motion?: { from: number; to: number; started: number };

  constructor(private send: (type: string, extra?: object) => void) {}

  document(
    version: number,
    markdown: string,
    maxLine: number,
    edit?: EditPosition,
  ) {
    if (this.version !== version) {
      this.starts = [0];
      for (let i = 0; i < markdown.length; i++)
        if (markdown[i] === "\n") this.starts.push(i + 1);
      this.pending = undefined;
      this.map = undefined;
      this.version = version;
      this.edit = undefined;
      this.motion = undefined;
    }
    if (edit?.version === version && edit.inputTime >= this.inputTime) {
      this.inputTime = edit.inputTime;
      this.ownsInput = false;
      this.pending = undefined;
      this.edit = edit;
    }
    if (this.maxLine !== maxLine) {
      this.maxLine = maxLine;
      this.map = undefined;
    }
  }

  attach(viewer: Viewer) {
    this.viewer = viewer;
    const container = viewer.element.parentElement!;
    const hover = (hovering: boolean) => {
      if (this.hovering === hovering) return;
      this.hovering = hovering;
      this.send("preview-hover", { hovering });
    };
    container.addEventListener("pointerenter", () => hover(true));
    container.addEventListener("pointerleave", () => hover(false));
    hover(container.matches(":hover"));
  }

  control() {
    this.inputTime = Math.max(Date.now(), this.inputTime + 1);
    this.pending = undefined;
    this.edit = undefined;
    this.motion = undefined;
    if (!this.ownsInput)
      this.send("preview-control", { inputTime: this.inputTime });
    this.ownsInput = true;
  }

  source(message: SourceScroll) {
    if (
      message.version !== this.version ||
      message.inputTime < this.inputTime ||
      message.request.generation <= this.generation
    )
      return;
    this.inputTime = message.inputTime;
    this.generation = message.request.generation;
    this.ownsInput = false;
    this.pending = message;
    this.edit = undefined;
    this.motion = undefined;
    if (this.maxLine !== message.maxLine) {
      this.maxLine = message.maxLine;
      this.map = undefined;
    }
  }

  private sourceLine(offset: number): number {
    let low = 0,
      high = this.starts.length;
    while (high - low > 1) {
      const middle = (low + high) >>> 1;
      if (this.starts[middle] <= offset) low = middle;
      else high = middle;
    }
    const end = this.starts[low + 1] ?? this.viewer!.getMarkdown().length + 1;
    return (
      low + (offset - this.starts[low]) / Math.max(1, end - this.starts[low])
    );
  }

  private mapping(): ScrollMap {
    const viewer = this.viewer!,
      engine = viewer.reader.markview;
    const revision = engine.stats().revision;
    if (
      !this.map ||
      this.revision !== revision ||
      this.maxScroll !== engine.maxScroll()
    ) {
      this.maxScroll = engine.maxScroll();
      this.revision = revision;
      const points = viewer.scrollAnchors().anchors.flatMap((anchor) => [
        { source: this.sourceLine(anchor.source.start), preview: anchor.top },
        { source: this.sourceLine(anchor.source.end), preview: anchor.bottom },
      ]);
      this.map = new ScrollMap(points, this.maxLine, engine.maxScroll());
    }
    return this.map;
  }

  frame() {
    const viewer = this.viewer;
    if (!viewer || viewer.reader.markview.stats().pending) return;
    if (this.pending) {
      const message = this.pending;
      this.pending = undefined;
      viewer.scrollTo(this.mapping().map("source", message.line));
    }
    if (this.edit) {
      const geometry = viewer.sourceToPreview(
        Math.min(
          this.edit.offset,
          Math.max(0, viewer.getMarkdown().length - 1),
        ),
      );
      this.edit = undefined;
      if (geometry) {
        const engine = viewer.reader.markview,
          from = engine.scroll(),
          height = viewer.element.clientHeight;
        const { y: top, height: lineHeight } = geometry.rect;
        const margin = Math.min(height / 4, Math.max(24, lineHeight * 2));
        const target =
          top < from
            ? top - margin
            : top + lineHeight > from + height
              ? top + lineHeight - height + margin
              : from;
        const to = Math.max(0, Math.min(engine.maxScroll(), target));
        if (Math.abs(from - to) > 1)
          this.motion = { from, to, started: performance.now() };
      }
    }
    if (this.motion) {
      const t = Math.min(1, (performance.now() - this.motion.started) / 100);
      viewer.scrollTo(
        this.motion.from +
          (this.motion.to - this.motion.from) * (1 - (1 - t) ** 3),
      );
      if (t === 1) this.motion = undefined;
    }
  }

  reading(position: ReadingPosition) {
    if (!this.viewer || !this.ownsInput || position.reason === "reflow") return;
    this.send("preview-scroll", {
      line: this.mapping().map("preview", this.viewer.reader.markview.scroll()),
      inputTime: this.inputTime,
    });
  }
}
