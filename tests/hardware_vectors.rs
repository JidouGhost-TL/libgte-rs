//! Retail PS1 GTE hardware verification tests.
//!
//! Test vectors are from the MIT-licensed `JaCzekanski/ps1-tests`
//! `gte/test-all` suite, captured on real retail PlayStation 1 hardware
//! (1150 tests: 50 register read/write tests and 50 tests for each of the
//! 22 GTE opcodes). See `tests.c` in that suite for provenance.
//!
//! This file contains a compact embedded subset that always runs, plus an
//! optional full-suite test: set `GTE_TESTS_C` to the path of the original
//! `tests.c` (3.87 MB) to run every hardware vector locally:
//!
//! ```sh
//! GTE_TESTS_C=/path/to/gte/test-all/tests.c cargo test --test hardware_vectors -- --nocapture
//! ```

use libgte::Gte;

/// One hardware test vector: 64 input registers (data 0..=31 then control
/// 0..=31), a command word (`0xFFFFFFFF` means "no command"), and the 64
/// expected register values afterwards.
pub struct HardwareTest {
    /// Test name from the original suite.
    pub name: &'static str,
    /// Input register file (64 words).
    pub input: [u32; 64],
    /// COP2 command word, or `0xFFFFFFFF` for register-only tests.
    pub opcode: u32,
    /// Expected register file after the optional command (64 words).
    pub output: [u32; 64],
}

mod common;

use common::HARDWARE_SUBSET;

/// Runs one hardware vector against a fresh [`Gte`] and returns a list of
/// `(register, expected, actual)` mismatches.
pub fn run_hardware_test(test: &HardwareTest) -> Vec<(usize, u32, u32)> {
    let mut gte = Gte::new();
    for (i, &value) in test.input.iter().enumerate() {
        if i < 32 {
            gte.write_data(i as u8, value);
        } else {
            gte.write_control((i - 32) as u8, value);
        }
    }
    if test.opcode != 0xFFFF_FFFF {
        gte.execute(test.opcode);
    }
    let mut mismatches = Vec::new();
    for (i, &expected) in test.output.iter().enumerate() {
        let actual = if i < 32 {
            gte.read_data(i as u8)
        } else {
            gte.read_control((i - 32) as u8)
        };
        if actual != expected {
            mismatches.push((i, expected, actual));
        }
    }
    mismatches
}

#[test]
fn embedded_hardware_subset_is_bit_exact() {
    let mut failed = 0;
    for test in HARDWARE_SUBSET {
        let mismatches = run_hardware_test(test);
        if !mismatches.is_empty() {
            failed += 1;
            eprintln!("FAIL {} (opcode {:#010x}):", test.name, test.opcode);
            for (reg, expected, actual) in &mismatches {
                eprintln!("  r{reg}: expected {expected:#010x}, got {actual:#010x}");
            }
        }
    }
    assert_eq!(
        failed,
        0,
        "{failed}/{} embedded hardware vectors failed",
        HARDWARE_SUBSET.len()
    );
}

/// Parses the original `tests.c` hardware suite (all `0x........` words in
/// file order: 64 inputs, 1 opcode, 64 outputs per test).
#[cfg(feature = "std")]
fn parse_full_suite(path: &str) -> Vec<(usize, [u32; 64], u32, [u32; 64])> {
    let text = std::fs::read_to_string(path).expect("read tests.c");
    let bytes = text.as_bytes();
    let mut words: Vec<u32> = Vec::new();
    let mut i = 0;
    while i + 10 <= bytes.len() {
        if bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
            let hex = &text[i + 2..i + 10];
            if hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                words.push(u32::from_str_radix(hex, 16).unwrap());
                i += 10;
                continue;
            }
        }
        i += 1;
    }
    // The file tail (`runTests`) contains two extra `0xffffffff` literals;
    // only complete 129-word test records are consumed.
    let mut tests = Vec::new();
    for (index, chunk) in words.chunks_exact(129).enumerate() {
        let mut input = [0u32; 64];
        let mut output = [0u32; 64];
        input.copy_from_slice(&chunk[0..64]);
        output.copy_from_slice(&chunk[65..129]);
        tests.push((index + 1, input, chunk[64], output));
    }
    tests
}

#[test]
#[cfg(feature = "std")]
fn full_retail_hardware_suite_is_bit_exact() {
    let Ok(path) = std::env::var("GTE_TESTS_C") else {
        eprintln!("GTE_TESTS_C not set; skipping full 1150-test retail suite");
        return;
    };
    let tests = parse_full_suite(&path);
    let mut passed = 0usize;
    let mut per_opcode: std::collections::BTreeMap<u32, (usize, usize)> =
        std::collections::BTreeMap::new();
    for (index, input, opcode, output) in &tests {
        let test = HardwareTest {
            name: "full",
            input: *input,
            opcode: *opcode,
            output: *output,
        };
        let mismatches = run_hardware_test(&test);
        let key = if *opcode == 0xFFFF_FFFF {
            0xFFFF_FFFF
        } else {
            opcode & 0x3F
        };
        let entry = per_opcode.entry(key).or_insert((0, 0));
        entry.1 += 1;
        if mismatches.is_empty() {
            passed += 1;
            entry.0 += 1;
        } else if entry.1 - entry.0 <= 60 {
            eprintln!(
                "FAIL test {index} opcode {opcode:#010x} ({} mismatches):",
                mismatches.len()
            );
            for (reg, expected, actual) in mismatches.iter().take(12) {
                eprintln!("  r{reg}: expected {expected:#010x}, got {actual:#010x}");
            }
        }
    }
    for (key, (ok, total)) in &per_opcode {
        eprintln!("opcode {key:#04x}: {ok}/{total} passed");
    }
    eprintln!("full suite: {passed}/{} passed", tests.len());
    assert_eq!(passed, tests.len(), "retail hardware parity incomplete");
}
