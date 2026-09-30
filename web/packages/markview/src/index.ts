// Public surface of `@markview/web`: `init()` plus the component classes.

import { default as __wbg_init, type InitInput } from "../wasm/markview_web.js";
import { LayoutUpdate } from "./layout-update.js";
import { Markview } from "./markview.js";
import { CanvasReader } from "./reader.js";
import type { CanvasReaderOptions } from "./reader.js";
import type { MarkviewOptions, MarkviewStats, Modifiers } from "./types.js";

export { CanvasReader, LayoutUpdate, Markview };
export type { CanvasReaderOptions, MarkviewOptions, MarkviewStats, Modifiers };

/** How `init()` finds the binary. */
export interface InitOptions {
	/**
	 * Where `markview_web_bg.wasm` is. Defaults to that file beside this
	 * module, which is where the package ships it. Set this when a bundler
	 * moves the JavaScript somewhere the binary does not follow.
	 */
	wasmUrl?: string | URL;
}

/** The one instantiation, so repeat `init()` calls share it. */
let initPromise: Promise<void> | null = null;

/**
 * Loads and instantiates the wasm binary the package ships. Call once before
 * anything else; repeat calls return the same promise.
 *
 * The binary is named beside this module rather than imported, because an
 * imported asset is only an asset to the bundler that resolves it: a published
 * entry point that already holds a rewritten filename string makes every
 * downstream bundler emit JavaScript alone and leave the binary behind. A
 * plain sibling reference survives bundling, and `wasmUrl` covers the case
 * where it does not.
 *
 * @throws when the binary is missing — build it with `scripts/build-web.sh`.
 */
export async function init(options?: InitOptions): Promise<void> {
	initPromise ??= (async () => {
		try {
			const source =
				options?.wasmUrl ??
				new URL("markview_web_bg.wasm", import.meta.url);
			// The glue wants `{ module_or_path }` when an argument is passed; a
			// bare string works but logs a deprecation warning.
			await __wbg_init({ module_or_path: source as InitInput });
		} catch (error) {
			initPromise = null;
			throw new Error(
				"the Markview wasm module could not be loaded; run scripts/build-web.sh "
				+ `and make sure markview_web_bg.wasm is served beside the module, or pass `
				+ `init({ wasmUrl }): ${String(error)}`,
				{ cause: error },
			);
		}
	})();
	await initPromise;
}

// `initSync` stays internal: the async `init()` is the supported entry point.
export default init;
