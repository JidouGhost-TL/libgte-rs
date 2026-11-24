//! Implementations of all 22 PlayStation 1 GTE COP2 opcodes.

use crate::registers::{Command, FLAG_DIVIDE_OVERFLOW, FLAG_IR0_SATURATED, FLAG_IR3_SATURATED};
use crate::unr::divide_unr;

use super::alu::{
    accumulate_mac, check_mac0_overflow, check_mac_overflow, multiply_matrix_vector,
    saturate_color, saturate_ir, saturate_screen, saturate_sz, sign_extend44,
};
use super::Gte;

impl Gte {
    #[inline(always)]
    fn vertex(&self, index: usize) -> [i16; 3] {
        [self.vx[index], self.vy[index], self.vz[index]]
    }

    #[inline(always)]
    fn set_mac0(&mut self, value: i64, flag: &mut u32) {
        check_mac0_overflow(flag, value);
        self.mac0 = value as i32;
    }

    #[inline(always)]
    fn push_sz(&mut self, value: i64, flag: &mut u32) {
        let clamped = saturate_sz(flag, value);
        self.sz[0] = self.sz[1];
        self.sz[1] = self.sz[2];
        self.sz[2] = self.sz[3];
        self.sz[3] = clamped;
    }

    #[inline(always)]
    pub(super) fn push_sxy_raw(&mut self, sx: i16, sy: i16) {
        self.sxy[0] = self.sxy[1];
        self.sxy[1] = self.sxy[2];
        self.sxy[2] = (sx, sy);
    }

    #[inline(always)]
    fn push_sxy_saturated(&mut self, sx: i32, sy: i32, flag: &mut u32) {
        let (csx, csy) = saturate_screen(flag, sx, sy);
        self.push_sxy_raw(csx, csy);
    }

    /// Pushes the colour FIFO from the current `MAC1..3` registers (`MACn >> 4`,
    /// saturated to `0..=255`).
    #[inline(always)]
    fn push_rgb_from_mac(&mut self, flag: &mut u32) {
        let r = saturate_color(flag, 0, self.mac[0] >> 4);
        let g = saturate_color(flag, 1, self.mac[1] >> 4);
        let b = saturate_color(flag, 2, self.mac[2] >> 4);
        let code = self.rgbc & 0xFF00_0000;
        self.rgb[0] = self.rgb[1];
        self.rgb[1] = self.rgb[2];
        self.rgb[2] = r | (g << 8) | (b << 16) | code;
    }

    #[inline]
    pub(super) fn op_rtps_vertex(
        &mut self,
        vertex: usize,
        sf: bool,
        lm: bool,
        calc_depth: bool,
        flag: &mut u32,
    ) {
        let vector = self.vertex(vertex);
        // In RTPS/RTPT the IR3 saturation flag comes from the MAC3 >> 12 (SZ)
        // path, not from the IR3 saturation itself; preserve any accumulated
        // bit from earlier vertices and recompute from sz_val below.
        let saved_ir3_flag = *flag & FLAG_IR3_SATURATED;
        let accs = multiply_matrix_vector(
            &self.rt,
            vector,
            Some(&self.tr),
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        let sz_val = accs[2] >> 12;
        *flag = (*flag & !FLAG_IR3_SATURATED)
            | saved_ir3_flag
            | (u32::from(!(-32768..=32767).contains(&sz_val)) << 22);
        self.push_sz(sz_val, flag);
        let (quotient, overflow) = divide_unr(self.h, self.sz[3]);
        if overflow {
            *flag |= FLAG_DIVIDE_OVERFLOW;
        }
        let q = i64::from(quotient);
        let sum_x = i64::from(self.ofx) + i64::from(self.ir[1]) * q;
        self.set_mac0(sum_x, flag);
        let sx = (sum_x >> 16) as i32;
        let sum_y = i64::from(self.ofy) + i64::from(self.ir[2]) * q;
        self.set_mac0(sum_y, flag);
        let sy = (sum_y >> 16) as i32;
        self.push_sxy_saturated(sx, sy, flag);
        if calc_depth {
            let depth = i64::from(self.dqb) + i64::from(self.dqa) * q;
            self.set_mac0(depth, flag);
            let depth_shifted = depth >> 12;
            let ir0 = depth_shifted.clamp(0, 4096);
            if ir0 != depth_shifted {
                *flag |= FLAG_IR0_SATURATED;
            }
            self.ir[0] = ir0 as i16;
        }
    }

    #[inline(always)]
    pub(super) fn op_nclip(&mut self, flag: &mut u32) {
        let (sx0, sy0) = (i64::from(self.sxy[0].0), i64::from(self.sxy[0].1));
        let (sx1, sy1) = (i64::from(self.sxy[1].0), i64::from(self.sxy[1].1));
        let (sx2, sy2) = (i64::from(self.sxy[2].0), i64::from(self.sxy[2].1));
        let value = sx0 * sy1 + sx1 * sy2 + sx2 * sy0 - sx0 * sy2 - sx1 * sy0 - sx2 * sy1;
        self.set_mac0(value, flag);
    }

    #[inline(always)]
    pub(super) fn op_op(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let shift: u32 = if sf { 12 } else { 0 };
        let ir1 = i64::from(self.ir[1]);
        let ir2 = i64::from(self.ir[2]);
        let ir3 = i64::from(self.ir[3]);
        // Outer product uses the diagonal of the rotation matrix on hardware.
        let d1 = i64::from(self.rt[0][0]);
        let d2 = i64::from(self.rt[1][1]);
        let d3 = i64::from(self.rt[2][2]);
        let products = [
            d2 * ir3 - d3 * ir2,
            d3 * ir1 - d1 * ir3,
            d1 * ir2 - d2 * ir1,
        ];
        for (i, &acc) in products.iter().enumerate() {
            check_mac_overflow(flag, i, acc);
            let mac_val = (sign_extend44(acc) >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
    }

    #[inline]
    pub(super) fn op_mvmva(&mut self, cmd: Command, flag: &mut u32) {
        let sf = cmd.sf();
        let lm = cmd.lm();
        let garbage;
        // For the reserved matrix (mx = 3) hardware feeds:
        //   row 0 = [-(R << 4), R << 4, IR0]
        //   row 1 = [RT13, RT13, RT13]
        //   row 2 = [RT22, RT22, RT22]
        let matrix: &[[i16; 3]; 3] = match cmd.mx() {
            0 => &self.rt,
            1 => &self.light,
            2 => &self.lcolor,
            _ => {
                let r = ((self.rgbc & 0xFF) as i16) << 4;
                garbage = [[-r, r, self.ir[0]], [self.rt[0][2]; 3], [self.rt[1][1]; 3]];
                &garbage
            }
        };
        let vector = match cmd.v() {
            0 => self.vertex(0),
            1 => self.vertex(1),
            2 => self.vertex(2),
            _ => [self.ir[1], self.ir[2], self.ir[3]],
        };
        let cv = cmd.cv();
        if cv != 2 {
            let translation = match cv {
                0 => Some(&self.tr),
                1 => Some(&self.bk),
                _ => None,
            };
            multiply_matrix_vector(
                matrix,
                vector,
                translation,
                sf,
                lm,
                &mut self.mac,
                &mut self.ir,
                flag,
            );
            return;
        }
        // Hardware bug: with cv = 2 (FC translation) the first product is
        // dropped from the stored MAC/IR values. Only the IR saturation flag
        // sees it (with `lm` forced off), driven by (FC << 12) + row[0] * v0.
        let shift: u32 = if sf { 12 } else { 0 };
        let v0 = i64::from(vector[0]);
        let v1 = i64::from(vector[1]);
        let v2 = i64::from(vector[2]);
        for (i, row) in matrix.iter().enumerate() {
            let base = i64::from(self.fc[i]) << 12;
            let first = accumulate_mac(flag, i, base + i64::from(row[0]) * v0);
            let _ = saturate_ir(flag, i, (first >> shift) as i32, false);
            let acc = accumulate_mac(flag, i, i64::from(row[1]) * v1);
            let acc = accumulate_mac(flag, i, acc + i64::from(row[2]) * v2);
            let mac_val = (acc >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
    }

    #[inline]
    pub(super) fn op_ncds(&mut self, vertex: usize, sf: bool, lm: bool, flag: &mut u32) {
        let vector = self.vertex(vertex);
        multiply_matrix_vector(
            &self.light,
            vector,
            None,
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        self.op_colour_depth(sf, lm, flag);
    }

    #[inline(always)]
    pub(super) fn op_cdp(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        self.op_colour_depth(sf, lm, flag);
    }

    /// Shared tail of `NCDS`/`CDP`/`NCDT`: colour-matrix multiply, then `DCPL`.
    #[inline(always)]
    fn op_colour_depth(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        multiply_matrix_vector(
            &self.lcolor,
            ir_vec,
            Some(&self.bk),
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        self.op_dcpl(sf, lm, flag);
    }

    /// `DCPL` (and tail of `NCDS`/`CDP`/`NCDT`): depth-cue from `(IR * RGBC) << 4`.
    #[inline(always)]
    pub(super) fn op_dcpl(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let bases = [
            (i64::from(self.ir[1]) * i64::from(self.rgbc & 0xFF)) << 4,
            (i64::from(self.ir[2]) * i64::from((self.rgbc >> 8) & 0xFF)) << 4,
            (i64::from(self.ir[3]) * i64::from((self.rgbc >> 16) & 0xFF)) << 4,
        ];
        self.op_depth_cue(bases, sf, lm, flag);
    }

    #[inline(always)]
    pub(super) fn op_nccs(&mut self, vertex: usize, sf: bool, lm: bool, flag: &mut u32) {
        let vector = self.vertex(vertex);
        multiply_matrix_vector(
            &self.light,
            vector,
            None,
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        self.op_cc(sf, lm, flag);
    }

    #[inline(always)]
    pub(super) fn op_cc(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        multiply_matrix_vector(
            &self.lcolor,
            ir_vec,
            Some(&self.bk),
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        self.multiply_by_rgbc_and_push(sf, lm, flag);
    }

    #[inline(always)]
    pub(super) fn op_ncs(&mut self, vertex: usize, sf: bool, lm: bool, flag: &mut u32) {
        let vector = self.vertex(vertex);
        multiply_matrix_vector(
            &self.light,
            vector,
            None,
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        multiply_matrix_vector(
            &self.lcolor,
            ir_vec,
            Some(&self.bk),
            sf,
            lm,
            &mut self.mac,
            &mut self.ir,
            flag,
        );
        self.push_rgb_from_mac(flag);
    }

    /// Final colour stage of `NCCS`/`CC`/`NCCT`: multiplies `IR1..3` by the
    /// `RGBC` colour bytes (`<< 4`), stores `MAC1..3` and `IR1..3`, and pushes
    /// the colour FIFO.
    #[inline(always)]
    fn multiply_by_rgbc_and_push(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let shift: u32 = if sf { 12 } else { 0 };
        for i in 0..3 {
            let byte = i64::from((self.rgbc >> (8 * i)) & 0xFF);
            let acc = (i64::from(self.ir[i + 1]) * byte) << 4;
            check_mac_overflow(flag, i, acc);
            let mac_val = (sign_extend44(acc) >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
        self.push_rgb_from_mac(flag);
    }

    #[inline(always)]
    pub(super) fn op_sqr(&mut self, sf: bool, lm: bool, flag: &mut u32) {
        let shift: u32 = if sf { 12 } else { 0 };
        for i in 0..3 {
            let v = i64::from(self.ir[i + 1]);
            let acc = v * v;
            check_mac_overflow(flag, i, acc);
            let mac_val = (sign_extend44(acc) >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
    }

    /// Two-step hardware depth-cue stage shared by `DPCS`, `DPCT`, `INTPL`,
    /// `DCPL`, `NCDS`, `CDP`, and `NCDT`.
    #[inline]
    pub(super) fn op_depth_cue(&mut self, bases: [i64; 3], sf: bool, lm: bool, flag: &mut u32) {
        let shift: u32 = if sf { 12 } else { 0 };
        let ir0 = i64::from(self.ir[0]);
        for (i, &base) in bases.iter().enumerate() {
            // Step 1: difference (FC << 12) - base, 44-bit MAC check, saturate to IR (lm = false).
            let diff_44 = (i64::from(self.fc[i]) << 12) - base;
            check_mac_overflow(flag, i, diff_44);
            let diff_mac = (sign_extend44(diff_44) >> shift) as i32;
            let diff_ir = saturate_ir(flag, i, diff_mac, false);
            // Step 2: interpolate base + IR0 * diff_ir, 44-bit MAC check, store MAC & IR (with lm).
            let mac_pre = base + ir0 * i64::from(diff_ir);
            check_mac_overflow(flag, i, mac_pre);
            let mac_val = (sign_extend44(mac_pre) >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
        self.push_rgb_from_mac(flag);
    }

    #[inline(always)]
    pub(super) fn op_avsz(&mut self, three: bool, flag: &mut u32) {
        let (factor, sum) = if three {
            (
                i64::from(self.zsf3),
                i64::from(self.sz[1]) + i64::from(self.sz[2]) + i64::from(self.sz[3]),
            )
        } else {
            (
                i64::from(self.zsf4),
                i64::from(self.sz[0])
                    + i64::from(self.sz[1])
                    + i64::from(self.sz[2])
                    + i64::from(self.sz[3]),
            )
        };
        let product = factor * sum;
        self.set_mac0(product, flag);
        self.otz = saturate_sz(flag, product >> 12);
    }

    #[inline(always)]
    pub(super) fn op_gpf(&mut self, sf: bool, lm: bool, with_base: bool, flag: &mut u32) {
        let ir0 = i64::from(self.ir[0]);
        let shift: u32 = if sf { 12 } else { 0 };
        for i in 0..3 {
            let base = if with_base {
                i64::from(self.mac[i]) << shift
            } else {
                0
            };
            let acc = base + ir0 * i64::from(self.ir[i + 1]);
            check_mac_overflow(flag, i, acc);
            let mac_val = (sign_extend44(acc) >> shift) as i32;
            self.mac[i] = mac_val;
            self.ir[i + 1] = saturate_ir(flag, i, mac_val, lm);
        }
        self.push_rgb_from_mac(flag);
    }
}
