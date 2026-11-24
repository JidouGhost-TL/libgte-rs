//! The GTE core: register file, typed helpers, save-state restoration, and COP2 dispatch.
//!
//! [`Gte`] is a plain value type (no heap allocation, `Send + Sync`) that
//! reproduces the register-transfer behaviour of the retail PlayStation 1
//! GTE (`CXD8530BQ` / `CXD8606BQ`) bit-exactly, including FIFO pushes,
//! saturation, 44-bit MAC overflow flags, the UNR reciprocal division, and
//! the documented `MVMVA` translation/matrix quirks.

mod alu;
mod ops;

use crate::registers::{Command, ControlReg, DataReg, FLAG_ERROR};
use alu::finalize_flag;

/// PlayStation 1 GTE (Geometry Transformation Engine, MIPS COP2) state.
///
/// All 32 data registers and 32 control registers live in this struct.
/// Create one with [`Gte::new`], load registers with
/// [`Gte::write_data`]/[`Gte::write_control`] (the `mtc2`/`ctc2`
/// equivalents), run a command with [`Gte::execute`], and read results back
/// with [`Gte::read_data`]/[`Gte::read_control`] (`mfc2`/`cfc2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gte {
    vx: [i16; 3],
    vy: [i16; 3],
    vz: [i16; 3],
    rgbc: u32,
    otz: u16,
    ir: [i16; 4],
    sxy: [(i16, i16); 3],
    sz: [u16; 4],
    rgb: [u32; 3],
    res1: u32,
    mac0: i32,
    mac: [i32; 3],
    lzcs: i32,
    lzcr: u32,
    rt: [[i16; 3]; 3],
    tr: [i32; 3],
    light: [[i16; 3]; 3],
    bk: [i32; 3],
    lcolor: [[i16; 3]; 3],
    fc: [i32; 3],
    ofx: i32,
    ofy: i32,
    h: u16,
    dqa: i16,
    dqb: i32,
    zsf3: i16,
    zsf4: i16,
    flag: u32,
}

impl Default for Gte {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
const fn pack_pair(lo: i16, hi: i16) -> u32 {
    (lo as u16 as u32) | ((hi as u16 as u32) << 16)
}

#[inline(always)]
const fn lo16(value: u32) -> i16 {
    value as u16 as i16
}

#[inline(always)]
const fn hi16(value: u32) -> i16 {
    (value >> 16) as u16 as i16
}

#[inline(always)]
const fn sign_extend16(value: u32) -> u32 {
    value as u16 as i16 as i32 as u32
}

impl Gte {
    /// Creates a new GTE with all registers zeroed (power-on reset state).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            vx: [0; 3],
            vy: [0; 3],
            vz: [0; 3],
            rgbc: 0,
            otz: 0,
            ir: [0; 4],
            sxy: [(0, 0); 3],
            sz: [0; 4],
            rgb: [0; 3],
            res1: 0,
            mac0: 0,
            mac: [0; 3],
            lzcs: 0,
            lzcr: 0,
            rt: [[0; 3]; 3],
            tr: [0; 3],
            light: [[0; 3]; 3],
            bk: [0; 3],
            lcolor: [[0; 3]; 3],
            fc: [0; 3],
            ofx: 0,
            ofy: 0,
            h: 0,
            dqa: 0,
            dqb: 0,
            zsf3: 0,
            zsf4: 0,
            flag: 0,
        }
    }

    /// Resets all registers to zero (identical to [`Gte::new`]).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Reads a data register (`mfc2` equivalent). `reg` is masked to 0..=31.
    ///
    /// Sign/zero extension, FIFO mirroring (`SXYP`), the `IRGB`/`ORGB`
    /// colour conversion, and the `LZCR` count all match retail hardware.
    #[inline]
    #[must_use]
    pub fn read_data(&self, reg: u8) -> u32 {
        match reg & 31 {
            0 => pack_pair(self.vx[0], self.vy[0]),
            1 => self.vz[0] as i32 as u32,
            2 => pack_pair(self.vx[1], self.vy[1]),
            3 => self.vz[1] as i32 as u32,
            4 => pack_pair(self.vx[2], self.vy[2]),
            5 => self.vz[2] as i32 as u32,
            6 => self.rgbc,
            7 => u32::from(self.otz),
            8 => self.ir[0] as i32 as u32,
            9 => self.ir[1] as i32 as u32,
            10 => self.ir[2] as i32 as u32,
            11 => self.ir[3] as i32 as u32,
            12 => pack_pair(self.sxy[0].0, self.sxy[0].1),
            13 => pack_pair(self.sxy[1].0, self.sxy[1].1),
            14 | 15 => pack_pair(self.sxy[2].0, self.sxy[2].1),
            16 => u32::from(self.sz[0]),
            17 => u32::from(self.sz[1]),
            18 => u32::from(self.sz[2]),
            19 => u32::from(self.sz[3]),
            20 => self.rgb[0],
            21 => self.rgb[1],
            22 => self.rgb[2],
            23 => self.res1,
            24 => self.mac0 as u32,
            25 => self.mac[0] as u32,
            26 => self.mac[1] as u32,
            27 => self.mac[2] as u32,
            28 | 29 => self.packed_ir_color(),
            30 => self.lzcs as u32,
            _ => self.lzcr,
        }
    }

    /// Writes a data register (`mtc2` equivalent). `reg` is masked to 0..=31.
    ///
    /// Writing `SXYP` pushes the screen XY FIFO, writing `IRGB` expands into
    /// `IR1..IR3`, and writing `LZCS` recomputes `LZCR`. Writes to `ORGB`
    /// and `LZCR` are ignored, as on hardware.
    #[inline]
    pub fn write_data(&mut self, reg: u8, value: u32) {
        match reg & 31 {
            0 => {
                self.vx[0] = lo16(value);
                self.vy[0] = hi16(value);
            }
            1 => self.vz[0] = lo16(value),
            2 => {
                self.vx[1] = lo16(value);
                self.vy[1] = hi16(value);
            }
            3 => self.vz[1] = lo16(value),
            4 => {
                self.vx[2] = lo16(value);
                self.vy[2] = hi16(value);
            }
            5 => self.vz[2] = lo16(value),
            6 => self.rgbc = value,
            7 => self.otz = value as u16,
            8 => self.ir[0] = lo16(value),
            9 => self.ir[1] = lo16(value),
            10 => self.ir[2] = lo16(value),
            11 => self.ir[3] = lo16(value),
            12 => self.sxy[0] = (lo16(value), hi16(value)),
            13 => self.sxy[1] = (lo16(value), hi16(value)),
            14 => self.sxy[2] = (lo16(value), hi16(value)),
            15 => self.push_sxy_raw(lo16(value), hi16(value)),
            16 => self.sz[0] = value as u16,
            17 => self.sz[1] = value as u16,
            18 => self.sz[2] = value as u16,
            19 => self.sz[3] = value as u16,
            20 => self.rgb[0] = value,
            21 => self.rgb[1] = value,
            22 => self.rgb[2] = value,
            23 => self.res1 = value,
            24 => self.mac0 = value as i32,
            25 => self.mac[0] = value as i32,
            26 => self.mac[1] = value as i32,
            27 => self.mac[2] = value as i32,
            28 => {
                self.ir[1] = ((value & 0x1F) << 7) as i16;
                self.ir[2] = (((value >> 5) & 0x1F) << 7) as i16;
                self.ir[3] = (((value >> 10) & 0x1F) << 7) as i16;
            }
            29 => {}
            30 => {
                self.lzcs = value as i32;
                self.lzcr = if self.lzcs >= 0 {
                    self.lzcs.leading_zeros()
                } else {
                    self.lzcs.leading_ones()
                };
            }
            _ => {}
        }
    }

    /// Reads a control register (`cfc2` equivalent). `reg` is masked to 0..=31.
    ///
    /// Paired matrix elements are returned packed; lone elements (`RT33`,
    /// `L33`, `LB3`) and the 16-bit coefficients (`H`, `DQA`, `ZSF3`,
    /// `ZSF4`) are sign-extended, exactly as on hardware.
    #[inline]
    #[must_use]
    pub fn read_control(&self, reg: u8) -> u32 {
        match reg & 31 {
            0 => pack_pair(self.rt[0][0], self.rt[0][1]),
            1 => pack_pair(self.rt[0][2], self.rt[1][0]),
            2 => pack_pair(self.rt[1][1], self.rt[1][2]),
            3 => pack_pair(self.rt[2][0], self.rt[2][1]),
            4 => sign_extend16(self.rt[2][2] as u16 as u32),
            5 => self.tr[0] as u32,
            6 => self.tr[1] as u32,
            7 => self.tr[2] as u32,
            8 => pack_pair(self.light[0][0], self.light[0][1]),
            9 => pack_pair(self.light[0][2], self.light[1][0]),
            10 => pack_pair(self.light[1][1], self.light[1][2]),
            11 => pack_pair(self.light[2][0], self.light[2][1]),
            12 => sign_extend16(self.light[2][2] as u16 as u32),
            13 => self.bk[0] as u32,
            14 => self.bk[1] as u32,
            15 => self.bk[2] as u32,
            16 => pack_pair(self.lcolor[0][0], self.lcolor[0][1]),
            17 => pack_pair(self.lcolor[0][2], self.lcolor[1][0]),
            18 => pack_pair(self.lcolor[1][1], self.lcolor[1][2]),
            19 => pack_pair(self.lcolor[2][0], self.lcolor[2][1]),
            20 => sign_extend16(self.lcolor[2][2] as u16 as u32),
            21 => self.fc[0] as u32,
            22 => self.fc[1] as u32,
            23 => self.fc[2] as u32,
            24 => self.ofx as u32,
            25 => self.ofy as u32,
            26 => sign_extend16(u32::from(self.h)),
            27 => sign_extend16(self.dqa as u16 as u32),
            28 => self.dqb as u32,
            29 => sign_extend16(self.zsf3 as u16 as u32),
            30 => sign_extend16(self.zsf4 as u16 as u32),
            _ => self.flag,
        }
    }

    /// Writes a control register (`ctc2` equivalent). `reg` is masked to 0..=31.
    ///
    /// Writing `FLAG` keeps only bits 12..=30 and recomputes the bit-31
    /// error summary, as on hardware.
    #[inline]
    pub fn write_control(&mut self, reg: u8, value: u32) {
        match reg & 31 {
            0 => {
                self.rt[0][0] = lo16(value);
                self.rt[0][1] = hi16(value);
            }
            1 => {
                self.rt[0][2] = lo16(value);
                self.rt[1][0] = hi16(value);
            }
            2 => {
                self.rt[1][1] = lo16(value);
                self.rt[1][2] = hi16(value);
            }
            3 => {
                self.rt[2][0] = lo16(value);
                self.rt[2][1] = hi16(value);
            }
            4 => self.rt[2][2] = lo16(value),
            5 => self.tr[0] = value as i32,
            6 => self.tr[1] = value as i32,
            7 => self.tr[2] = value as i32,
            8 => {
                self.light[0][0] = lo16(value);
                self.light[0][1] = hi16(value);
            }
            9 => {
                self.light[0][2] = lo16(value);
                self.light[1][0] = hi16(value);
            }
            10 => {
                self.light[1][1] = lo16(value);
                self.light[1][2] = hi16(value);
            }
            11 => {
                self.light[2][0] = lo16(value);
                self.light[2][1] = hi16(value);
            }
            12 => self.light[2][2] = lo16(value),
            13 => self.bk[0] = value as i32,
            14 => self.bk[1] = value as i32,
            15 => self.bk[2] = value as i32,
            16 => {
                self.lcolor[0][0] = lo16(value);
                self.lcolor[0][1] = hi16(value);
            }
            17 => {
                self.lcolor[0][2] = lo16(value);
                self.lcolor[1][0] = hi16(value);
            }
            18 => {
                self.lcolor[1][1] = lo16(value);
                self.lcolor[1][2] = hi16(value);
            }
            19 => {
                self.lcolor[2][0] = lo16(value);
                self.lcolor[2][1] = hi16(value);
            }
            20 => self.lcolor[2][2] = lo16(value),
            21 => self.fc[0] = value as i32,
            22 => self.fc[1] = value as i32,
            23 => self.fc[2] = value as i32,
            24 => self.ofx = value as i32,
            25 => self.ofy = value as i32,
            26 => self.h = value as u16,
            27 => self.dqa = lo16(value),
            28 => self.dqb = value as i32,
            29 => self.zsf3 = lo16(value),
            30 => self.zsf4 = lo16(value),
            _ => {
                self.flag = finalize_flag(value & 0x7FFF_F000);
            }
        }
    }

    /// Reads a data register via a typed [`DataReg`] identifier.
    #[inline]
    #[must_use]
    pub fn read_data_reg(&self, reg: DataReg) -> u32 {
        self.read_data(reg.index())
    }

    /// Writes a data register via a typed [`DataReg`] identifier.
    #[inline]
    pub fn write_data_reg(&mut self, reg: DataReg, value: u32) {
        self.write_data(reg.index(), value);
    }

    /// Reads a control register via a typed [`ControlReg`] identifier.
    #[inline]
    #[must_use]
    pub fn read_control_reg(&self, reg: ControlReg) -> u32 {
        self.read_control(reg.index())
    }

    /// Writes a control register via a typed [`ControlReg`] identifier.
    #[inline]
    pub fn write_control_reg(&mut self, reg: ControlReg, value: u32) {
        self.write_control(reg.index(), value);
    }

    /// Returns a snapshot of all 32 data registers as read via `mfc2`.
    #[must_use]
    pub fn data_registers(&self) -> [u32; 32] {
        let mut out = [0u32; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.read_data(i as u8);
        }
        out
    }

    /// Returns a snapshot of all 32 control registers as read via `cfc2`.
    #[must_use]
    pub fn control_registers(&self) -> [u32; 32] {
        let mut out = [0u32; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.read_control(i as u8);
        }
        out
    }

    /// Restores the full GTE register state from a snapshot captured by
    /// [`Gte::data_registers`] and [`Gte::control_registers`].
    ///
    /// Unlike sequential `write_data` calls, this restores registers
    /// losslessly without triggering `SXYP` FIFO pushes, `IRGB` expansion
    /// over `IR1..IR3`, or `LZCR` recomputation.
    pub fn restore_registers(&mut self, data: &[u32; 32], control: &[u32; 32]) {
        self.vx = [lo16(data[0]), lo16(data[2]), lo16(data[4])];
        self.vy = [hi16(data[0]), hi16(data[2]), hi16(data[4])];
        self.vz = [lo16(data[1]), lo16(data[3]), lo16(data[5])];
        self.rgbc = data[6];
        self.otz = data[7] as u16;
        self.ir = [lo16(data[8]), lo16(data[9]), lo16(data[10]), lo16(data[11])];
        self.sxy = [
            (lo16(data[12]), hi16(data[12])),
            (lo16(data[13]), hi16(data[13])),
            (lo16(data[14]), hi16(data[14])),
        ];
        self.sz = [
            data[16] as u16,
            data[17] as u16,
            data[18] as u16,
            data[19] as u16,
        ];
        self.rgb = [data[20], data[21], data[22]];
        self.res1 = data[23];
        self.mac0 = data[24] as i32;
        self.mac = [data[25] as i32, data[26] as i32, data[27] as i32];
        self.lzcs = data[30] as i32;
        self.lzcr = data[31];
        for (i, &value) in control.iter().enumerate() {
            self.write_control(i as u8, value);
        }
    }

    /// Sets the 3x3 rotation matrix (`RT11..RT33`, 1.3.12 fixed point).
    #[inline]
    pub fn set_rotation_matrix(&mut self, matrix: [[i16; 3]; 3]) {
        self.rt = matrix;
    }

    /// Sets the translation vector (`TRX`, `TRY`, `TRZ`).
    #[inline]
    pub fn set_translation(&mut self, translation: [i32; 3]) {
        self.tr = translation;
    }

    /// Sets vertex `index` (`0..3`) to `[x, y, z]`.
    ///
    /// # Panics
    /// Panics if `index >= 3`.
    #[inline]
    pub fn set_vertex(&mut self, index: usize, vertex: [i16; 3]) {
        self.vx[index] = vertex[0];
        self.vy[index] = vertex[1];
        self.vz[index] = vertex[2];
    }

    /// Returns the 3-entry screen XY FIFO (`[SXY0, SXY1, SXY2]`).
    #[inline]
    #[must_use]
    pub const fn screen_xy(&self) -> [(i16, i16); 3] {
        self.sxy
    }

    /// Returns the 4-entry screen Z FIFO (`[SZ0, SZ1, SZ2, SZ3]`).
    #[inline]
    #[must_use]
    pub const fn screen_z(&self) -> [u16; 4] {
        self.sz
    }

    /// Returns the 3-entry colour FIFO (`[RGB0, RGB1, RGB2]`).
    #[inline]
    #[must_use]
    pub const fn rgb_fifo(&self) -> [u32; 3] {
        self.rgb
    }

    /// Returns the ordering-table Z register (`OTZ`).
    #[inline]
    #[must_use]
    pub const fn otz(&self) -> u16 {
        self.otz
    }

    /// Returns the 32-bit `MAC0` register.
    #[inline]
    #[must_use]
    pub const fn mac0(&self) -> i32 {
        self.mac0
    }

    /// Returns the current `FLAG` control register (including bit 31).
    #[inline]
    #[must_use]
    pub const fn flag(&self) -> u32 {
        self.flag
    }

    /// Returns `true` if the `FLAG` bit-31 error summary is set.
    #[inline]
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.flag & FLAG_ERROR != 0
    }

    #[inline(always)]
    fn packed_ir_color(&self) -> u32 {
        // Each 5-bit channel is IRn >> 7 saturated to 0..=31.
        let r = (i32::from(self.ir[1]) >> 7).clamp(0, 31) as u32;
        let g = (i32::from(self.ir[2]) >> 7).clamp(0, 31) as u32;
        let b = (i32::from(self.ir[3]) >> 7).clamp(0, 31) as u32;
        r | (g << 5) | (b << 10)
    }

    /// Executes one COP2 command word (the 25-bit `cop2` immediate).
    ///
    /// Hardware clears `FLAG` at the start of every COP2 command and
    /// recomputes the bit-31 error summary at the end. Function code `0x00`
    /// executes as the undocumented hardware mirror of `RTPS` (`0x01`); all
    /// other undefined/reserved function codes clear `FLAG` to `0` and leave
    /// other registers unchanged.
    #[inline]
    pub fn execute(&mut self, command: u32) {
        let cmd = Command::new(command);
        let sf = cmd.sf();
        let lm = cmd.lm();
        let mut flag = 0u32;
        match cmd.opcode_code() {
            0x00 | 0x01 => self.op_rtps_vertex(0, sf, lm, true, &mut flag),
            0x06 => self.op_nclip(&mut flag),
            0x0C => self.op_op(sf, lm, &mut flag),
            0x10 => {
                let bases = [
                    i64::from(self.rgbc & 0xFF) << 16,
                    i64::from((self.rgbc >> 8) & 0xFF) << 16,
                    i64::from((self.rgbc >> 16) & 0xFF) << 16,
                ];
                self.op_depth_cue(bases, sf, lm, &mut flag);
            }
            0x11 => {
                let bases = [
                    i64::from(self.ir[1]) << 12,
                    i64::from(self.ir[2]) << 12,
                    i64::from(self.ir[3]) << 12,
                ];
                self.op_depth_cue(bases, sf, lm, &mut flag);
            }
            0x12 => self.op_mvmva(cmd, &mut flag),
            0x13 => self.op_ncds(0, sf, lm, &mut flag),
            0x14 => self.op_cdp(sf, lm, &mut flag),
            0x16 => {
                for v in 0..3 {
                    self.op_ncds(v, sf, lm, &mut flag);
                }
            }
            0x1B => self.op_nccs(0, sf, lm, &mut flag),
            0x1C => self.op_cc(sf, lm, &mut flag),
            0x1E => self.op_ncs(0, sf, lm, &mut flag),
            0x20 => {
                for v in 0..3 {
                    self.op_ncs(v, sf, lm, &mut flag);
                }
            }
            0x28 => self.op_sqr(sf, lm, &mut flag),
            0x29 => self.op_dcpl(sf, lm, &mut flag),
            0x2A => {
                for _ in 0..3 {
                    let fifo = self.rgb[0];
                    let bases = [
                        i64::from(fifo & 0xFF) << 16,
                        i64::from((fifo >> 8) & 0xFF) << 16,
                        i64::from((fifo >> 16) & 0xFF) << 16,
                    ];
                    self.op_depth_cue(bases, sf, lm, &mut flag);
                }
            }
            0x2D => self.op_avsz(true, &mut flag),
            0x2E => self.op_avsz(false, &mut flag),
            0x30 => {
                self.op_rtps_vertex(0, sf, lm, false, &mut flag);
                self.op_rtps_vertex(1, sf, lm, false, &mut flag);
                self.op_rtps_vertex(2, sf, lm, true, &mut flag);
            }
            0x3D => self.op_gpf(sf, lm, false, &mut flag),
            0x3E => self.op_gpf(sf, lm, true, &mut flag),
            0x3F => {
                for v in 0..3 {
                    self.op_nccs(v, sf, lm, &mut flag);
                }
            }
            _ => {}
        }
        self.flag = finalize_flag(flag);
    }

    /// Executes a decoded [`Command`].
    #[inline]
    pub fn execute_command(&mut self, command: Command) {
        self.execute(command.raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registers::{
        Opcode, FLAG_IR1_SATURATED, FLAG_IR2_SATURATED, FLAG_IR3_SATURATED,
        FLAG_MAC1_POSITIVE_OVERFLOW, FLAG_SZ3_OTZ_SATURATED,
    };

    #[test]
    fn dcpl_sf0_uses_exact_base_not_sf1_endpoint_shortcut() {
        // sf = 0, FC = 0, IR1 = 1, R = 1:
        // base = (1 * 1) << 4 = 16; diff_44 = 0 - 16 = -16; diff_ir = -16 (not -4096).
        let mut gte = Gte::new();
        gte.write_data(8, 2); // IR0 = 2
        gte.write_data(9, 1); // IR1 = 1
        gte.write_data(6, 1); // RGBC: R = 1, G = 0, B = 0
        gte.execute_command(Command::encode(Opcode::Dcpl, false, false, 0, 0, 0));
        // MAC1 = 16 + 2 * (-16) = -16 (old buggy shortcut gave 16 + 2 * (-4096) = -8176).
        assert_eq!(gte.read_data(25) as i32, -16);
        assert_eq!(gte.read_data(9) as i32, -16);

        // Also check IR0 = 1: MAC1 = 16 + 1 * (-16) = 0.
        gte.reset();
        gte.write_data(8, 1); // IR0 = 1
        gte.write_data(9, 1); // IR1 = 1
        gte.write_data(6, 1); // R = 1
        gte.execute_command(Command::encode(Opcode::Dcpl, false, false, 0, 0, 0));
        assert_eq!(gte.read_data(25) as i32, 0);
        assert_eq!(gte.read_data(9) as i32, 0);
    }

    #[test]
    fn dcpl_sf1_negative_ir_lm_true_has_no_spurious_ir_saturation_flag() {
        // sf = 1, IR1 = -100, R = 16, FC = 1000, IR0 = 0x1000, lm = true:
        // base = (-100 * 16) << 4 = -25600
        // diff_44 = (1000 << 12) - (-25600) = 4_121_600 -> >> 12 = 1006 (no saturation)
        // mac_pre = -25600 + 4096 * 1006 = 4_094_976 -> >> 12 = 999 (positive, no saturation)
        let mut gte = Gte::new();
        gte.write_data(8, 0x1000); // IR0
        gte.write_data(9, (-100i16) as u16 as u32); // IR1
        gte.write_data(10, (-50i16) as u16 as u32); // IR2
        gte.write_data(11, (-25i16) as u16 as u32); // IR3
        gte.write_data(6, 0x0010_1010); // R = 16, G = 16, B = 16
        gte.write_control(21, 1000); // RFC
        gte.write_control(22, 1000); // GFC
        gte.write_control(23, 1000); // BFC
        gte.execute_command(Command::encode(Opcode::Dcpl, true, true, 0, 0, 0));
        assert_eq!(gte.read_data(25) as i32, 999);
        assert_eq!(gte.read_data(9) as i32, 999);
        assert_eq!(
            gte.flag() & (FLAG_IR1_SATURATED | FLAG_IR2_SATURATED | FLAG_IR3_SATURATED),
            0,
            "unexpected IR saturation flag: {:#010x}",
            gte.flag()
        );
    }

    #[test]
    fn gpl_sf1_detects_44bit_mac_overflow() {
        let mut gte = Gte::new();
        gte.write_data(25, i32::MAX as u32); // MAC1 = i32::MAX
        gte.write_data(8, 4096); // IR0 = 4096
        gte.write_data(9, 4096); // IR1 = 4096
        gte.execute_command(Command::encode(Opcode::Gpl, true, false, 0, 0, 0));
        assert_ne!(gte.flag() & FLAG_MAC1_POSITIVE_OVERFLOW, 0);
        assert!(gte.has_error());
    }

    #[test]
    fn mvmva_sf0_followed_by_gpl_does_not_leak_upper_44bit_carry() {
        // Produce a MAC1 > i32::MAX in MVMVA(sf=0) so bits 32..43 were non-zero
        // before truncation to 32-bit architectural MAC1.
        let mut gte = Gte::new();
        gte.set_rotation_matrix([[0x7FFF, 0, 0], [0, 0, 0], [0, 0, 0]]);
        gte.set_translation([0x0010_0000, 0, 0]); // TRX << 12 = 0x1_0000_0000 (bit 32 set)
        gte.set_vertex(0, [1, 0, 0]);
        gte.execute_command(Command::encode(Opcode::Mvmva, false, false, 0, 0, 0));
        let mac1_after_mvmva = gte.read_data(25) as i32;
        assert_eq!(mac1_after_mvmva, 0x7FFF);

        // Now run GPL(sf=0) with IR0 = 1, IR1 = 1. If upper 44-bit carry had
        // leaked, GPL would see 0x1_0000_7FFF instead of 0x7FFF.
        gte.write_data(8, 1);
        gte.write_data(9, 1);
        gte.execute_command(Command::encode(Opcode::Gpl, false, false, 0, 0, 0));
        assert_eq!(gte.read_data(25) as i32, 0x8000);
        assert_eq!(gte.flag() & FLAG_MAC1_POSITIVE_OVERFLOW, 0);
    }

    #[test]
    fn unknown_opcode_clears_flag_and_preserves_other_registers() {
        let mut gte = Gte::new();
        gte.write_data(0, 0x1234_5678);
        gte.write_data(25, 42);
        gte.write_control(31, FLAG_MAC1_POSITIVE_OVERFLOW);
        assert!(gte.has_error());
        let data_before = gte.data_registers();

        // Opcode 0x02 is undefined/reserved.
        gte.execute(0x0008_0002);
        assert_eq!(gte.flag(), 0);
        assert!(!gte.has_error());
        assert_eq!(gte.data_registers(), data_before);
    }

    #[test]
    fn opcode_0x00_mirrors_rtps() {
        let mut a = Gte::new();
        a.set_rotation_matrix([[0x1000, 0, 0], [0, 0x1000, 0], [0, 0, 0x1000]]);
        a.set_translation([0, 0, 200]);
        a.set_vertex(0, [40, -20, 50]);
        a.write_control(24, 160 << 16);
        a.write_control(25, 120 << 16);
        a.write_control(26, 300);
        let mut b = a.clone();

        a.execute(0x0008_0000); // 0x00 mirror with sf = 1
        b.execute(0x0008_0001); // RTPS (0x01) with sf = 1
        assert_eq!(a, b);
    }

    #[test]
    fn rtpt_state_equals_freshly_restored_registers() {
        let mut gte = Gte::new();
        gte.set_rotation_matrix([[0x1000, 0, 0], [0, 0x1000, 0], [0, 0, 0x1000]]);
        gte.set_translation([0, 0, 100]);
        gte.set_vertex(0, [10, 20, 30]);
        gte.set_vertex(1, [40, 50, 60]);
        gte.set_vertex(2, [70, 80, 90]);
        gte.write_control(24, 160 << 16);
        gte.write_control(25, 120 << 16);
        gte.write_control(26, 300);
        gte.write_control(27, 0x7FFF); // DQA
        gte.write_control(28, 0x7FFF_0000); // DQB (causes MAC0/IR0 saturation on v2)
        gte.execute_command(Command::encode(Opcode::Rtpt, true, false, 0, 0, 0));

        let data = gte.data_registers();
        let control = gte.control_registers();
        let mut restored = Gte::new();
        restored.restore_registers(&data, &control);
        assert_eq!(gte, restored);
    }

    #[test]
    fn restore_registers_is_lossless_including_side_effect_registers() {
        let mut gte = Gte::new();
        // Power-on reset round-trip (lzcs = 0, lzcr = 0).
        let mut restored = Gte::new();
        restored.write_data(30, 12345);
        restored.restore_registers(&gte.data_registers(), &gte.control_registers());
        assert_eq!(gte, restored);

        // Non-trivial state with arbitrary IR1..3 (not multiples of 128),
        // distinct SXY0/1/2, non-zero LZCS/LZCR, and FLAG bits.
        gte.write_data(9, 123);
        gte.write_data(10, -456i16 as u16 as u32);
        gte.write_data(11, 789);
        gte.write_data(12, 0x0001_0002);
        gte.write_data(13, 0x0003_0004);
        gte.write_data(14, 0x0005_0006);
        gte.write_data(30, 0x00F0_0000);
        gte.write_control(31, FLAG_IR1_SATURATED);

        let mut restored2 = Gte::new();
        restored2.restore_registers(&gte.data_registers(), &gte.control_registers());
        assert_eq!(gte, restored2);
    }

    #[test]
    fn mvmva_matrix_vector_translation_variants() {
        let mut gte = Gte::new();
        // Light matrix (mx = 1) = diag(0x1000, 0x2000, 0x0800)
        gte.write_control(8, 0x1000);
        gte.write_control(10, 0x2000);
        gte.write_control(12, 0x0800);
        // LColor matrix (mx = 2) = diag(0x0800, 0x0800, 0x0800)
        gte.write_control(16, 0x0800);
        gte.write_control(18, 0x0800);
        gte.write_control(20, 0x0800);
        // BK (cv = 1) = [10, 20, 30]
        gte.write_control(13, 10);
        gte.write_control(14, 20);
        gte.write_control(15, 30);
        // Vertex 2 (v = 2) = [100, -50, 200]
        gte.set_vertex(2, [100, -50, 200]);

        // mx = 1 (light), v = 2 (V2), cv = 1 (BK), sf = 1, lm = 0
        gte.execute_command(Command::encode(Opcode::Mvmva, true, false, 1, 2, 1));
        assert_eq!(gte.read_data(25) as i32, 110);
        assert_eq!(gte.read_data(26) as i32, -80);
        assert_eq!(gte.read_data(27) as i32, 130);
        assert_eq!(gte.read_data(10) as i32, -80); // lm = 0 preserves negative

        // mx = 2 (lcolor), v = 3 (IR), cv = 3 (none), sf = 1, lm = 1
        gte.execute_command(Command::encode(Opcode::Mvmva, true, true, 2, 3, 3));
        assert_eq!(gte.read_data(25) as i32, 55);
        assert_eq!(gte.read_data(26) as i32, -40);
        assert_eq!(gte.read_data(27) as i32, 65);
        assert_eq!(gte.read_data(10) as i32, 0); // lm = 1 clamps negative to 0

        // mx = 3 (garbage matrix), v = 0, cv = 0 (TR = 0)
        gte.write_data(6, 0x0000_0010); // R = 16 -> R << 4 = 256
        gte.write_data(8, 0x1000); // IR0 = 4096
        gte.write_control(1, 0x0000_0800); // RT13 = 0x0800
        gte.write_control(2, 0x0000_0400); // RT22 = 0x0400
        gte.set_vertex(0, [16, 32, 64]);
        gte.execute_command(Command::encode(Opcode::Mvmva, true, false, 3, 0, 0));
        // row 0: (-256*16 + 256*32 + 4096*64) >> 12 = (4096 + 262144) >> 12 = 65
        // row 1: (0x800 * (16 + 32 + 64)) >> 12 = (2048 * 112) >> 12 = 56
        // row 2: (0x400 * (16 + 32 + 64)) >> 12 = (1024 * 112) >> 12 = 28
        assert_eq!(gte.read_data(25) as i32, 65);
        assert_eq!(gte.read_data(26) as i32, 56);
        assert_eq!(gte.read_data(27) as i32, 28);
    }

    #[test]
    fn lzcs_lzcr_edge_cases() {
        let mut gte = Gte::new();
        gte.write_data(30, 0);
        assert_eq!(gte.read_data(31), 32);
        gte.write_data(30, u32::MAX); // -1
        assert_eq!(gte.read_data(31), 32);
        gte.write_data(30, i32::MIN as u32); // 0x8000_0000
        assert_eq!(gte.read_data(31), 1);
        gte.write_data(30, i32::MAX as u32); // 0x7FFF_FFFF
        assert_eq!(gte.read_data(31), 1);
        // LZCR is read-only.
        gte.write_data(31, 99);
        assert_eq!(gte.read_data(31), 1);
    }

    #[test]
    fn irgb_orgb_and_sxyp_register_behaviour() {
        let mut gte = Gte::new();
        // IRGB write expands 5-bit channels by << 7 into IR1..3.
        gte.write_data(28, (31 << 10) | (15 << 5) | 3);
        assert_eq!(gte.read_data(9), 3 << 7);
        assert_eq!(gte.read_data(10), 15 << 7);
        assert_eq!(gte.read_data(11), 31 << 7);
        assert_eq!(gte.read_data(28), (31 << 10) | (15 << 5) | 3);
        assert_eq!(gte.read_data(29), (31 << 10) | (15 << 5) | 3);

        // ORGB is read-only and clamps out-of-range IR1..3 to 0..=31.
        gte.write_data(9, (-100i16) as u16 as u32);
        gte.write_data(10, 0x7FFF);
        gte.write_data(11, 16 << 7);
        gte.write_data(29, 0);
        assert_eq!(gte.read_data(29), (16 << 10) | (31 << 5));

        // SXYP pushes FIFO on write and mirrors SXY2 on read.
        gte.write_data(15, 0x000A_0001);
        gte.write_data(15, 0x000B_0002);
        gte.write_data(15, 0x000C_0003);
        assert_eq!(gte.screen_xy(), [(1, 10), (2, 11), (3, 12)]);
        assert_eq!(gte.read_data(15), 0x000C_0003);
    }

    #[test]
    fn dpct_and_avsz3_avsz4() {
        let mut gte = Gte::new();
        // DPCT: depth-cue 3 RGB FIFO entries with IR0 = 0x0800 (0.5), FC = [255, 0, 0].
        gte.write_data(6, 0x2000_0000); // CODE = 0x20
        gte.write_data(20, 0x0000_0000); // RGB0 = black
        gte.write_data(21, 0x0000_0080); // RGB1 = R=128
        gte.write_data(22, 0x0000_00FF); // RGB2 = R=255
        gte.write_data(8, 0x0800); // IR0 = 0.5
        gte.write_control(21, 0x0FF0); // RFC = 255 << 4
        gte.execute_command(Command::encode(Opcode::Dpct, true, false, 0, 0, 0));
        let fifo = gte.rgb_fifo();
        assert_eq!(fifo[0] >> 24, 0x20);
        assert_eq!(fifo[1] >> 24, 0x20);
        assert_eq!(fifo[2] >> 24, 0x20);
        assert_eq!(fifo[0] & 0xFF, 127);
        assert_eq!(fifo[1] & 0xFF, 191);
        assert_eq!(fifo[2] & 0xFF, 255);

        // AVSZ3 & AVSZ4
        gte.write_data(16, 100); // SZ0
        gte.write_data(17, 200); // SZ1
        gte.write_data(18, 300); // SZ2
        gte.write_data(19, 400); // SZ3
        gte.write_control(29, 0x1000); // ZSF3 = 1.0
        gte.write_control(30, 0x0800); // ZSF4 = 0.5
        gte.execute_command(Command::encode(Opcode::Avsz3, true, false, 0, 0, 0));
        assert_eq!(gte.mac0(), 900 << 12);
        assert_eq!(gte.otz(), 900);
        assert_eq!(gte.flag() & FLAG_SZ3_OTZ_SATURATED, 0);

        gte.execute_command(Command::encode(Opcode::Avsz4, true, false, 0, 0, 0));
        assert_eq!(gte.mac0(), 1000 * 0x0800);
        assert_eq!(gte.otz(), 500);
        assert_eq!(gte.screen_z(), [100, 200, 300, 400]);
    }

    #[test]
    fn opcode_cycles_match_ps1_hardware() {
        assert_eq!(Opcode::Rtps.cycles(), 15);
        assert_eq!(Opcode::Nclip.cycles(), 8);
        assert_eq!(Opcode::Op.cycles(), 6);
        assert_eq!(Opcode::Dpcs.cycles(), 8);
        assert_eq!(Opcode::Intpl.cycles(), 8);
        assert_eq!(Opcode::Mvmva.cycles(), 8);
        assert_eq!(Opcode::Ncds.cycles(), 19);
        assert_eq!(Opcode::Cdp.cycles(), 13);
        assert_eq!(Opcode::Ncdt.cycles(), 44);
        assert_eq!(Opcode::Nccs.cycles(), 17);
        assert_eq!(Opcode::Cc.cycles(), 11);
        assert_eq!(Opcode::Ncs.cycles(), 14);
        assert_eq!(Opcode::Nct.cycles(), 30);
        assert_eq!(Opcode::Sqr.cycles(), 5);
        assert_eq!(Opcode::Dcpl.cycles(), 8);
        assert_eq!(Opcode::Dpct.cycles(), 17);
        assert_eq!(Opcode::Avsz3.cycles(), 5);
        assert_eq!(Opcode::Avsz4.cycles(), 6);
        assert_eq!(Opcode::Rtpt.cycles(), 23);
        assert_eq!(Opcode::Gpf.cycles(), 5);
        assert_eq!(Opcode::Gpl.cycles(), 5);
        assert_eq!(Opcode::Ncct.cycles(), 39);
    }
}
