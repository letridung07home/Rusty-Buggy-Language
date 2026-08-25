//! Coverage-guided fuzz target over the public `evaluate` entry point.
//!
//! libFuzzer drives this with arbitrary bytes; the evaluator must never
//! panic, overflow, or hang on any input. Bytes are converted lossily to
//! UTF-8 so the target exercises the same boundary as the fuzz-smoke harness
//! in `tests/fuzz_smoke.rs`, while libFuzzer's coverage feedback steers
//! toward deep lexer, parser, and evaluator states that random bytes rarely
//! reach.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rusty_buggy_language::evaluate;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let _ = evaluate(&input);
});
