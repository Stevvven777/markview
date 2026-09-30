// `CanvasReader`: the convenience that keeps a host page a wiring file. It
// owns the frame loop (advance a pending layout, then present), the
// device-pixel-ratio sizing, the wheel/pointer/keyboard bindings and
// `Ctrl`/`Cmd`+`C` copying.

import { LayoutUpdate } from "./layout-update.js";
import { Markview } from "./markview.js";
import type { MarkviewOptions, MarkviewStats } from "./types.js";

/** Options for `CanvasReader.attach`; every key is optional. */
export interface CanvasReaderOptions {
	/** Document laid out progressively when the reader attaches. */
	markdown?: string;
	/** Options handed to the engine. */
	markview?: MarkviewOptions;
	/** Layout budget per animation frame (8). */
	stepBudgetMs?: number;
	/** Called with the stats of every presented frame. */
	onStats?: (stats: MarkviewStats) => void;
	/** Called once when the frame loop stops on an error. */
	onError?: (error: unknown) => void;
}

/**
 * A canvas bound to the engine plus the input handlers a reader needs.
 *
 * Attach with `CanvasReader.attach(canvas, options)` after `init()`. The
 * rAF loop starts immediately and runs until `destroy()`.
 */
export class CanvasReader {
	/** The underlying `Markview` handle, for direct access. */
	readonly markview: Markview;

	private canvas: HTMLCanvasElement;
	private stepBudgetMs: number;
	private onStats: ((stats: MarkviewStats) => void) | undefined;
	private onError: ((error: unknown) => void) | undefined;
	private cssWidth = 0;
	private cssHeight = 0;
	private cssDpr = 0;
	private raf = 0;
	private stopped = false;
	/** Set once the handle is freed, so `destroy()` stays idempotent even
	 * after the loop stopped on an error and the page must still clean up. */
	private disposed = false;
	/** Aborted by `destroy()`, which detaches every canvas listener at once. */
	private listeners = new AbortController();
	/** Whether this reader is the one that made the canvas focusable. */
	private madeFocusable = false;
	/** The sizing attributes the pin replaced, so `destroy()` can put the
	 * canvas' own box back before it takes the pin off. */
	private pinnedBox: PinnedBox | null = null;
	/** The resumable layout the loop advances; one document at a time. */
	private update: LayoutUpdate | null = null;

	private constructor(
		markview: Markview,
		canvas: HTMLCanvasElement,
		options: CanvasReaderOptions,
		pinnedBox: PinnedBox | null,
	) {
		this.markview = markview;
		this.canvas = canvas;
		this.stepBudgetMs = options.stepBudgetMs ?? 8;
		this.onStats = options.onStats;
		this.onError = options.onError;
		this.pinnedBox = pinnedBox;

		// A canvas is not focusable unless something says so, and the keyboard
		// bindings below only fire on a focused element. A caller's own
		// `tabindex` is left exactly as it is.
		if (!canvas.hasAttribute("tabindex")) {
			canvas.tabIndex = 0;
			this.madeFocusable = true;
		}
		this.bindInput();
		if (options.markdown !== undefined) {
			this.setMarkdown(options.markdown);
		}
		this.raf = requestAnimationFrame(this.loop);
	}

	/**
	 * Creates the engine for `canvas` and starts the reader's frame loop.
	 *
	 * The canvas box is pinned before the engine writes the backing store: a
	 * canvas the page sizes with its attributes takes its box from those very
	 * attributes, so the first write would resize the element that measured it
	 * and every later frame would double it again.
	 */
	static async attach(canvas: HTMLCanvasElement, options: CanvasReaderOptions = {}): Promise<CanvasReader> {
		const pinnedBox = pinLogicalSize(canvas);
		const markview = await Markview.create(canvas, options.markview);
		return new CanvasReader(markview, canvas, options, pinnedBox);
	}

	/** Replaces the document and starts a progressive layout. */
	setMarkdown(markdown: string): void {
		this.#live();
		this.update = this.markview.beginLayout(markdown);
	}

	/**
	 * Stops the frame loop, detaches the canvas listeners and releases the wasm
	 * handle. The listener teardown comes first: a pointer event arriving after
	 * the handle is freed would otherwise reach a destroyed `Markview`.
	 */
	destroy(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.stopped = true;
		cancelAnimationFrame(this.raf);
		this.listeners.abort();
		if (this.madeFocusable) {
			this.canvas.removeAttribute("tabindex");
		}
		if (this.pinnedBox) {
			// The engine left the backing store at device-pixel size, and on a
			// canvas the page sizes by its attributes those attributes are the
			// box. Put the originals back before the pin comes off, so teardown
			// leaves the element the size it was found at.
			this.canvas.width = this.pinnedBox.width;
			this.canvas.height = this.pinnedBox.height;
			this.canvas.style.removeProperty("width");
			this.canvas.style.removeProperty("height");
		}
		this.markview.destroy();
	}

	/** The loop: fit, advance a pending layout, present, report, repeat. */
	private loop = (): void => {
		if (this.stopped) return;
		try {
			if (this.fit()) {
				// A narrower column reflowed the document, which replaced the
				// pass any `LayoutUpdate` was driving.
				this.update = null;
			}
			if (this.update && !this.update.done) {
				this.update.step(this.stepBudgetMs);
			} else {
				this.update = null;
				// A reflow is a pass of its own, which no `LayoutUpdate` owns,
				// so the loop advances it here. This is a no-op otherwise.
				this.markview.stepPending(this.stepBudgetMs);
			}
			const stats = this.markview.frame();
			this.onStats?.(stats);
		} catch (error) {
			// A lost surface must stop the loop, not raise once per frame.
			this.stopped = true;
			if (this.onError) {
				this.onError(error);
			} else {
				console.error("markview reader:", error);
			}
			return;
		}
		this.raf = requestAnimationFrame(this.loop);
	};

	/**
	 * Sizes the backing store when the canvas box or the device ratio changes,
	 * and reports whether that narrowed the reading column into a reflow.
	 * `attach` has already pinned the box of a canvas the page sizes with its
	 * attributes, so the writes below cannot feed back into the size that
	 * measured them.
	 */
	private fit(): boolean {
		const rect = this.canvas.getBoundingClientRect();
		const dpr = window.devicePixelRatio || 1;
		if (rect.width === this.cssWidth && rect.height === this.cssHeight && dpr === this.cssDpr) return false;
		this.cssWidth = rect.width;
		this.cssHeight = rect.height;
		this.cssDpr = dpr;
		this.canvas.width = Math.max(1, Math.round(rect.width * dpr));
		this.canvas.height = Math.max(1, Math.round(rect.height * dpr));
		return this.markview.resize(rect.width, rect.height, dpr);
	}

	/** Pointer, wheel and keyboard bindings over the canvas. */
	private bindInput(): void {
		const canvas = this.canvas;
		// One signal for the set, so `destroy()` detaches them together.
		const signal = this.listeners.signal;
		canvas.addEventListener("pointerdown", (event) => {
			if (event.button !== 0) return;
			event.preventDefault();
			canvas.focus({ preventScroll: true });
			canvas.setPointerCapture(event.pointerId);
			this.markview.pointerDown(event.offsetX, event.offsetY, {
				shift: event.shiftKey,
				control: event.ctrlKey,
				alt: event.altKey,
				meta: event.metaKey,
			});
		}, { signal });
		canvas.addEventListener("pointermove", (event) => {
			this.markview.pointerMove(event.offsetX, event.offsetY);
		}, { signal });
		canvas.addEventListener("pointerup", (event) => {
			this.markview.pointerUp(event.offsetX, event.offsetY);
		}, { signal });
		canvas.addEventListener("pointercancel", (event) => {
			this.markview.pointerUp(event.offsetX, event.offsetY);
		}, { signal });
		canvas.addEventListener("wheel", (event) => {
			event.preventDefault();
			// Firefox reports line deltas; the engine scrolls in CSS pixels.
			const dy = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
			this.markview.scrollBy(dy);
		}, { passive: false, signal });
		canvas.addEventListener("keydown", (event) => {
			if (!(event.ctrlKey || event.metaKey)) return;
			const key = event.key.toLowerCase();
			if (key === "c") {
				event.preventDefault();
				void this.markview.copy();
			} else if (key === "a") {
				event.preventDefault();
				this.markview.selectAll();
			}
		}, { signal });
	}

	/** Guards every entry point after `destroy()`. */
	#live(): void {
		if (this.disposed) {
			throw new Error("this CanvasReader has been destroyed");
		}
	}
}

/** The sizing attributes a pinned canvas has to get back on teardown. */
interface PinnedBox {
	width: number;
	height: number;
}

/**
 * Pins the measured CSS size of a canvas whose box comes from its `width` and
 * `height` attributes, so writing the backing store cannot resize the element
 * that was measured. Returns the attributes the pin has to restore, or `null`
 * when nothing was pinned; a canvas a stylesheet sizes is left to that
 * stylesheet and keeps following it.
 */
function pinLogicalSize(canvas: HTMLCanvasElement): PinnedBox | null {
	if (canvas.style.width !== "" || canvas.style.height !== "") return null;
	const rect = canvas.getBoundingClientRect();
	const width = canvas.width;
	const height = canvas.height;
	if (Math.round(rect.width) !== width || Math.round(rect.height) !== height) return null;
	// The write is the probe: an attribute-driven box moves with it, a
	// stylesheet-sized one does not. Restore the attributes either way, because
	// the engine sizes the backing store after this and owns it from then on.
	canvas.width = width + 1;
	canvas.height = height + 1;
	const probed = canvas.getBoundingClientRect();
	canvas.width = width;
	canvas.height = height;
	if (probed.width === rect.width && probed.height === rect.height) return null;
	canvas.style.width = `${rect.width}px`;
	canvas.style.height = `${rect.height}px`;
	return { width, height };
}

