import type { Viewer, ReadingPosition } from "@markview/viewer";
import { ScrollMap, type SyncRequest } from "@markview/scroll-sync";

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

  constructor(private send: (type: string, extra?: object) => void) {}

  document(version: number, markdown: string, maxLine: number) {
    if (this.version !== version) {
      this.starts = [0];
      for (let i = 0; i < markdown.length; i++)
        if (markdown[i] === "\n") this.starts.push(i + 1);
      this.pending = undefined;
      this.map = undefined;
      this.version = version;
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
    if (
      !this.viewer ||
      this.viewer.reader.markview.stats().pending ||
      !this.pending
    )
      return;
    const message = this.pending;
    this.pending = undefined;
    this.viewer.scrollTo(this.mapping().map("source", message.line));
  }

  reading(position: ReadingPosition) {
    if (!this.viewer || !this.ownsInput || position.reason === "reflow") return;
    this.send("preview-scroll", {
      line: this.mapping().map("preview", this.viewer.reader.markview.scroll()),
      inputTime: this.inputTime,
    });
  }
}
