// Public types for `@markview/web`: the options the engine accepts and the
// statistics every read-back returns.

/** Layout and typography options; every key is optional with a default. */
export interface MarkviewOptions {
	/** Layout column width, logical px. (760) */
	width?: number;
	/** Base body size, logical px. (18) */
	fontSize?: number;
	/** Paper theme. ("light") */
	theme?: "light" | "dark";
	/** Justify body lines. (true) */
	justify?: boolean;
	/** Hyphenate across line breaks. (true) */
	hyphenate?: boolean;
	/** First-line indent of a paragraph, in multiples of the font size. (0) */
	paragraphIndent?: number;
	/** Greedy line breaking instead of optimum. (false) */
	greedy?: boolean;
	/** Treat leading front matter as metadata instead of content. (false) */
	hideFrontMatter?: boolean;
	/** Heading printed above hidden front matter. ("Metadata") */
	frontMatterLabel?: string;
}

/** The engine's counters, as returned by every stats read-back. */
export interface MarkviewStats {
	/** Bumped on every published snapshot. */
	revision: number;
	/** Blocks in the published snapshot. */
	blocks: number;
	/** Laid-out document height, logical px. */
	contentHeight: number;
	/** The reading column's width, logical px. It narrows to fit the canvas. */
	width: number;
	/** Blocks the last pass reused from the previous one. */
	reused: number;
	/** Last parse time, ms. */
	parseMs: number;
	/** Last layout time, ms. */
	layoutMs: number;
	/** Last presented frame time, ms. */
	frameMs: number;
	/** Frames presented so far. */
	frames: number;
	/** Glyphs in the atlas. */
	glyphs: number;
	/** Rendering backend, e.g. `"Gl"`. */
	backend: string;
	/** GPU adapter name. */
	adapter: string;
	/** UTF-16 code units in the current selection, like `String.length`. */
	selectionLength: number;
	/** Whether a started layout still has blocks to lay out. */
	pending: boolean;
}

/** Pointer modifier state for `Markview.pointerDown`. */
export interface Modifiers {
	shift: boolean;
	control: boolean;
	alt: boolean;
	meta: boolean;
}
