//! Hardware reciprocal (UNR) division table and division routine.
//!
//! The GTE divides with a 257-entry reciprocal approximation table plus a
//! single Newton–Raphson refinement step, as documented in PSX-SPX ("GTE
//! Division Inaccuracy"). This module reproduces the retail hardware table
//! and algorithm, including the overflow behaviour (`0x1FFFF` quotient and
//! the `FLAG` divide-overflow bit).

/// The 257-entry hardware UNR reciprocal table (byte values).
///
/// Entry `i` covers normalised divisors in
/// `[0x7FC0 + i * 0x80, 0x7FC0 + (i + 1) * 0x80)`; the reciprocal seed used
/// by the hardware is `UNR_TABLE[i] + 0x101`, an approximation of
/// `0x1000000 / (0x8000 + i * 0x80)` in the range `0x101..=0x200`. Entries
/// saturate at the ends (`0xFF` for `i = 0`, `0x00` for the top entries),
/// matching the retail `CXD8530BQ`/`CXD8606BQ` table.
pub const UNR_TABLE: [u8; 257] = build_unr_table();

const fn build_unr_table() -> [u8; 257] {
    let mut table = [0u8; 257];
    let mut i = 0usize;
    while i < 257 {
        let divisor = 0x8000u32 + (i as u32) * 0x80;
        // Reciprocal seed, rounded to nearest: 0x1000000 / divisor.
        let seed = (0x0100_0000u32 + (divisor >> 1)) / divisor;
        table[i] = if seed <= 0x101 {
            0
        } else if seed >= 0x200 {
            0xFF
        } else {
            (seed - 0x101) as u8
        };
        i += 1;
    }
    table
}

/// Divides `numerator` by `denominator` with the hardware UNR algorithm.
///
/// Returns `(quotient, overflow)`. On overflow (a zero divisor, or
/// `numerator >= denominator * 2`) the quotient is `0x1FFFF` and
/// `overflow` is `true`, matching the hardware `FLAG` bit-17 behaviour.
/// Inputs are the 16-bit `H` and `SZ3` values; the quotient is the 17-bit
/// `(H << 16) / SZ` approximation produced by the reciprocal table and one
/// Newton–Raphson refinement step, exactly as on retail hardware.
#[must_use]
pub fn divide_unr(numerator: u32, denominator: u32) -> (u32, bool) {
    if denominator == 0 || numerator >= denominator.wrapping_mul(2) {
        return (0x1_FFFF, true);
    }
    // Normalise the divisor into [0x8000, 0x10000) and shift the numerator
    // by the same amount.
    let shift = denominator.leading_zeros() - 16;
    let d = denominator << shift;
    let index = ((d - 0x7FC0) >> 7) as usize;
    let u = u64::from(UNR_TABLE[index]) + 0x101;
    let d = u64::from(d);
    // One Newton–Raphson refinement of the reciprocal seed, in the
    // hardware's fixed-point form: refined = u * (2 - d * u).
    let e = (0x20_00080u64 - (d * u)) >> 8;
    let refined = (0x80u64 + (e * u)) >> 8;
    let n = u64::from(numerator) << shift;
    let quotient = ((n * refined) + 0x8000) >> 16;
    (quotient.min(0x1_FFFF) as u32, false)
}
