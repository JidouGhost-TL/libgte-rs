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

/// Divides 16-bit `h` by 16-bit `sz3` with the hardware UNR algorithm.
///
/// Returns `(quotient, overflow)`. On overflow (a zero divisor, or
/// `h >= sz3 * 2`) the quotient is `0x1FFFF` and `overflow` is `true`,
/// matching the hardware `FLAG` bit-17 behaviour. The quotient is the
/// 17-bit `(H << 16) / SZ3` approximation produced by the reciprocal table
/// and one Newton–Raphson refinement step, exactly as on retail hardware.
#[inline]
#[must_use]
pub const fn divide_unr(h: u16, sz3: u16) -> (u32, bool) {
    let numerator = h as u32;
    let denominator = sz3 as u32;
    if denominator * 2 <= numerator {
        return (0x1_FFFF, true);
    }
    // Normalise the 16-bit divisor into [0x8000, 0xFFFF] and shift the
    // numerator by the same amount. Because `d` is guaranteed to be in
    // 0x8000..=0xFFFF, `index` is always in 0..=256.
    let shift = sz3.leading_zeros();
    let d = denominator << shift;
    let index = ((d - 0x7FC0) >> 7) as usize;
    let u = (UNR_TABLE[index] as u64) + 0x101;
    let d = d as u64;
    // One Newton–Raphson refinement of the reciprocal seed, in the
    // hardware's fixed-point form: refined = u * (2 - d * u).
    let e = (0x20_00080u64 - (d * u)) >> 8;
    let refined = (0x80u64 + (e * u)) >> 8;
    let n = (numerator as u64) << shift;
    let quotient = ((n * refined) + 0x8000) >> 16;
    let clamped = if quotient > 0x1_FFFF {
        0x1_FFFF
    } else {
        quotient as u32
    };
    (clamped, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divide_unr_zero_divisor_overflows() {
        assert_eq!(divide_unr(0, 0), (0x1_FFFF, true));
        assert_eq!(divide_unr(1, 0), (0x1_FFFF, true));
        assert_eq!(divide_unr(u16::MAX, 0), (0x1_FFFF, true));
    }

    #[test]
    fn divide_unr_small_divisor_and_overflow_boundary() {
        assert_eq!(divide_unr(0, 1), (0, false));
        assert_eq!(divide_unr(1, 1), (0x1_0000, false));
        assert_eq!(divide_unr(2, 1), (0x1_FFFF, true));
        assert_eq!(divide_unr(200, 100), (0x1_FFFF, true));
        assert_eq!(divide_unr(199, 100), (130_417, false));
    }

    #[test]
    fn divide_unr_max_u16_inputs() {
        // u16::MAX / u16::MAX returns 0xFFFF under hardware UNR without panic or overflow.
        let (q, ovf) = divide_unr(u16::MAX, u16::MAX);
        assert!(!ovf);
        assert_eq!(q, 0xFFFF);
        assert_eq!(divide_unr(0, u16::MAX), (0, false));
    }

    #[test]
    fn divide_unr_mid_range_matches_hardware() {
        assert_eq!(divide_unr(32767, 26330), (81559, false));
    }
}
