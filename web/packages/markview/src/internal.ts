// Shared internals: options serialization, stats parsing and the module-wide
// wasm init promise.

import type { MarkviewOptions, MarkviewStats } from "./types.js";

/**
 * Serializes options into the JSON string the wasm glue accepts. `undefined`
 * becomes `null`, which tells the engine to use its built-in defaults.
 */
export function serializeOptions(options?: MarkviewOptions): string | null {
	if (!options) return null;
	return JSON.stringify(options);
}

/**
 * Parses a `Stats` JSON string from the glue into `MarkviewStats`. A malformed
 * read-back is a bug in the engine, so it throws rather than faking numbers.
 */
export function parseStats(json: string): MarkviewStats {
	return JSON.parse(json) as MarkviewStats;
}
