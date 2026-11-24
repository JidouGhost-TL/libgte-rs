//! Low-level GTE arithmetic, saturation, overflow, and matrix-vector ALU helpers.

use crate::registers::FLAG_ERROR_MASK;

/// 44-bit signed MAC maximum (`2^43 - 1`).
pub(super) const MAC44_MAX: i64 = 0x7FF_FFFF_FFFF;
/// 44-bit signed MAC minimum (`-2^43`).
pub(super) const MAC44_MIN: i64 = -0x800_0000_0000;

/// Sign-extends the low 44 bits of `value` to `i64`.
#[inline(always)]
pub(super) const fn sign_extend44(value: i64) -> i64 {
    (value << 20) >> 20
}

/// Raises the `MAC1..3` positive/negative overflow bits if `value` leaves
/// the signed 44-bit range.
///
/// Channel `index` (0..=2) maps to bits 30..=28 (positive) and 27..=25 (negative).
#[inline(always)]
pub(super) fn check_mac_overflow(flag: &mut u32, index: usize, value: i64) {
    *flag |= u32::from(value > MAC44_MAX) << (30 - index);
    *flag |= u32::from(value < MAC44_MIN) << (27 - index);
}

/// Raises the `MAC0` positive/negative overflow bits if `value` leaves the
/// signed 32-bit range.
#[inline(always)]
pub(super) fn check_mac0_overflow(flag: &mut u32, value: i64) {
    *flag |= u32::from(value > i64::from(i32::MAX)) << 16;
    *flag |= u32::from(value < i64::from(i32::MIN)) << 15;
}

/// Performs one wrapping 44-bit MAC accumulation step: checks overflow on
/// `value` and sign-extends the low 44 bits.
#[inline(always)]
pub(super) fn accumulate_mac(flag: &mut u32, index: usize, value: i64) -> i64 {
    check_mac_overflow(flag, index, value);
    sign_extend44(value)
}

/// Saturates `value` into `IR1..3` (channel `index` 0..=2 -> `FLAG` bits 24..=22).
#[inline(always)]
pub(super) fn saturate_ir(flag: &mut u32, index: usize, value: i32, lm: bool) -> i16 {
    let lo = if lm { 0 } else { -32768 };
    let clamped = value.clamp(lo, 32767);
    *flag |= u32::from(clamped != value) << (24 - index);
    clamped as i16
}

/// Saturates a colour component to `0..=255` (channel `index` 0..=2 -> `FLAG` bits 21..=19).
#[inline(always)]
pub(super) fn saturate_color(flag: &mut u32, index: usize, value: i32) -> u32 {
    let clamped = value.clamp(0, 255);
    *flag |= u32::from(clamped != value) << (21 - index);
    clamped as u32
}

/// Saturates screen `(sx, sy)` coordinates to `-1024..=1023` (`FLAG` bits 14 and 13).
#[inline(always)]
pub(super) fn saturate_screen(flag: &mut u32, sx: i32, sy: i32) -> (i16, i16) {
    let csx = sx.clamp(-1024, 1023);
    let csy = sy.clamp(-1024, 1023);
    *flag |= u32::from(csx != sx) << 14;
    *flag |= u32::from(csy != sy) << 13;
    (csx as i16, csy as i16)
}

/// Saturates a Z value (`SZ3` / `OTZ`) to `0..=0xFFFF` (`FLAG` bit 18).
#[inline(always)]
pub(super) fn saturate_sz(flag: &mut u32, value: i64) -> u16 {
    let clamped = value.clamp(0, 0xFFFF);
    *flag |= u32::from(clamped != value) << 18;
    clamped as u16
}

/// Recomputes `FLAG` bit 31 (error summary) from [`FLAG_ERROR_MASK`].
#[inline(always)]
pub(super) const fn finalize_flag(flag: u32) -> u32 {
    flag | ((if flag & FLAG_ERROR_MASK != 0 {
        1u32
    } else {
        0u32
    }) << 31)
}

/// Multiplies `matrix` by `vector` (plus optional translation), storing
/// `MAC1..3` (`sf`-shifted) and `IR1..3`. Returns the signed 44-bit
/// accumulators before the `sf` shift (used by `RTPS`/`RTPT` for `SZ3`).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn multiply_matrix_vector(
    matrix: &[[i16; 3]; 3],
    vector: [i16; 3],
    translation: Option<&[i32; 3]>,
    sf: bool,
    lm: bool,
    mac: &mut [i32; 3],
    ir: &mut [i16; 4],
    flag: &mut u32,
) -> [i64; 3] {
    let shift: u32 = if sf { 12 } else { 0 };
    let v0 = i64::from(vector[0]);
    let v1 = i64::from(vector[1]);
    let v2 = i64::from(vector[2]);
    let mut accs = [0i64; 3];
    for i in 0..3 {
        let row = &matrix[i];
        let mut acc: i64 = match translation {
            Some(t) => i64::from(t[i]) << 12,
            None => 0,
        };
        acc = accumulate_mac(flag, i, acc + i64::from(row[0]) * v0);
        acc = accumulate_mac(flag, i, acc + i64::from(row[1]) * v1);
        acc = accumulate_mac(flag, i, acc + i64::from(row[2]) * v2);
        accs[i] = acc;
        let mac_val = (acc >> shift) as i32;
        mac[i] = mac_val;
        ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
    }
    accs
}
