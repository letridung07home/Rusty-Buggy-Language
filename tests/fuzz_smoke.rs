//! A lightweight, dependency-free fuzzing harness over the public
//! `evaluate` entry point.
//!
//! The test is `#[ignore]`d by default so the ordinary test suite stays fast;
//! CI runs it explicitly under a time bound. Its sole job is to prove the
//! evaluator never panics (and never hangs on the generated inputs), whether
//! the input is a plausible program or arbitrary UTF-8.

use rusty_buggy_language::evaluate;

/// Deterministic xorshift64 PRNG so failures are reproducible from the seed.
struct Prng(u64);

impl Prng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        self.next() as usize % bound
    }
}

/// A byte-oriented source alphabet covering every token the lexer recognizes,
/// plus syntactic glue, whitespace, and comment markers.
const SOURCE_ALPHABET: &[u8] =
    b"letabcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_+-*/%()<>=!; \n\t\r/";

/// Programs that stress otherwise-rare code paths.
const EDGE_PROGRAMS: &[&str] = &[
    "-9223372036854775808",
    "-9223372036854775808 / -1",
    "-9223372036854775808 % -1",
    "9223372036854775807 + 1",
    "--9223372036854775808",
    "0 / 0",
    "1 % 0",
    "1 /* unterminated",
    "1 < 2 < 3",
    "let a = 1; let a = 2; a",
];

fn fuzz_iterations(mut prng: Prng, target: usize) {
    for _ in 0..target {
        let kind = prng.below(4);

        let input = match kind {
            0 => {
                // A random-length sequence of lexer characters.
                let length = prng.below(200);
                let mut bytes = Vec::with_capacity(length);
                for _ in 0..length {
                    bytes.push(SOURCE_ALPHABET[prng.below(SOURCE_ALPHABET.len())]);
                }
                bytes
            }
            1 => {
                // A hand-rolled pseudo-program with random literal values,
                // reusing a few operators to interleave valid and invalid.
                let a = prng.below(20_000).to_string();
                let b = prng.below(20_000).to_string();
                let operator = source_of(prng.below(3), &["+", "-", "*", "/", "%", "<", ">"]);
                let prefix = if prng.below(2) == 0 { "-" } else { "" };
                format!("let x = {}{}; x {operator} {}", prefix, a, b).into_bytes()
            }
            2 => {
                // Arbitrary valid UTF-8 drawn from a wide character set,
                // including multi-byte code points.
                let length = prng.below(80);
                let mut chars = Vec::with_capacity(length);
                for _ in 0..length {
                    chars.push(char::from_u32(0x20 + prng.below(0x3000) as u32).unwrap_or(' '));
                }
                chars.into_iter().collect::<String>().into_bytes()
            }
            _ => {
                // Every other iteration, run a targeted edge case verbatim.
                prune_edge(&mut prng).as_bytes().to_vec()
            }
        };

        // The evaluator must never panic or hang, regardless of validity.
        let input = String::from_utf8_lossy(&input);
        let _ = evaluate(&input);
    }
}

fn source_of<'a>(index: usize, options: &[&'a str]) -> &'a str {
    options[index % options.len()]
}

fn prune_edge(prng: &mut Prng) -> &'static str {
    EDGE_PROGRAMS[prng.below(EDGE_PROGRAMS.len())]
}

/// Runs the harness for a fixed number of inputs. Marked `#[ignore]` so the
/// normal test suite skips it; CI invokes it explicitly under a time bound.
#[test]
#[ignore = "long-running fuzz smoke; run explicitly in CI with a timeout"]
fn evaluator_never_panics_on_fuzzed_input() {
    const ITERATIONS: usize = 200_000;
    fuzz_iterations(Prng::new(0xC0FFEE), ITERATIONS);
}

/// A fast regression run that always executes so the default test suite still
/// sees some mutation-based coverage.
#[test]
fn evaluator_never_panics_on_small_fuzz_batch() {
    const ITERATIONS: usize = 2_000;
    fuzz_iterations(Prng::new(0x5EED), ITERATIONS);
}
