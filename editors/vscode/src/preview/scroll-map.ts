import type { Viewer } from "@markview/viewer";
import { ScrollMap, type ScrollPoint } from "@markview/scroll-sync";

/** Interpolate source gaps and progress through images and wrapped lines. */
export class PreviewScrollMap {
  private revision = -1;
  private map: ScrollMap | undefined;

  get(viewer: Viewer): ScrollMap {
    const engine = viewer.reader.markview;
    const revision = engine.stats().revision;
    if (this.map && revision === this.revision) return this.map;
    const points: ScrollPoint[] = [];
    const anchors = viewer.scrollAnchors().anchors;
    for (const anchor of anchors) {
      points.push(
        { source: anchor.source.start, preview: anchor.top },
        { source: anchor.source.end, preview: anchor.bottom },
      );
    }
    this.revision = revision;
    return (this.map = new ScrollMap(
      points,
      viewer.getMarkdown().length,
      engine.contentHeight(),
    ));
  }
}
