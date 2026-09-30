//! G4, Tier 1 + Tier 2: the ratex math path, including its panic-catch
//! fallback. Oracle (O1, O2): no uncaught panic or overflow within budgets;
//! under ASAN, a clean run. `Err` is the legal outcome for invalid input.
#![no_main]

use libfuzzer_sys::fuzz_target;
use markview_core::limits::Limits;
use markview_core::math::MathEngine;
use mvfuzz::{budget, oracle, ratex};

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::parse().from_env();
	let latex = String::from_utf8_lossy(data);
	if latex.len() > 256 * 1024 {
		return;
	}
	// `ratex` 0.1.14 overflows `i64` on a wide `\char` literal; the reader
	// never sees the panic but libFuzzer's abort hook makes it fatal here.
	// See `mvfuzz::ratex`, including how to remove this allowance.
	ratex::allow_char_overflow();
	let guard = budget::InputGuard::new();
	let seed = oracle::derive(data);
	// Sizes vary normally, and one in eight takes a raw bit pattern: tiny,
	// huge, or denormal, to stress the metric scaling.
	let size = if seed & 7 == 0 {
		f32::from_bits(seed as u32)
	} else {
		oracle::f32_unit(seed as u32) * 12.0
	};
	let display = seed & 1 == 0;
	let limits = Limits {
		math_formula_bytes: 64 + (seed >> 4) as usize % (256 * 1024),
		math_bytes: 1024 + (seed >> 16) as usize % (8 * 1024 * 1024),
		..Default::default()
	};
	let mut engine = MathEngine::default();
	engine.set_limits(limits);
	if let Ok(box_) = engine.layout(&latex, display, size) {
		oracle::assert_bounded("math.width", box_.width);
		oracle::assert_bounded("math.ascent", box_.ascent);
		oracle::assert_bounded("math.descent", box_.descent);
	}
	guard.finish(&budget, latex.len());
});
