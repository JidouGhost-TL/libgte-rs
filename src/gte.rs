//! The GTE core: register file and COP2 command execution.
//!
//! [`Gte`] is a plain value type (no heap allocation, `Send + Sync`) that
//! reproduces the register-transfer behaviour of the retail PlayStation 1
//! GTE (`CXD8530BQ` / `CXD8606BQ`) bit-exactly, including FIFO pushes,
//! saturation, 44-bit MAC overflow flags, the UNR reciprocal division, and
//! the documented `MVMVA` translation/matrix quirks.

use crate::registers::{
    Command, ControlReg, DataReg, Opcode, FLAG_COLOR_B_SATURATED, FLAG_COLOR_G_SATURATED,
    FLAG_COLOR_R_SATURATED, FLAG_DIVIDE_OVERFLOW, FLAG_ERROR, FLAG_ERROR_MASK, FLAG_IR0_SATURATED,
    FLAG_IR1_SATURATED, FLAG_IR2_SATURATED, FLAG_IR3_SATURATED, FLAG_MAC0_NEGATIVE_OVERFLOW,
    FLAG_MAC0_POSITIVE_OVERFLOW, FLAG_MAC1_NEGATIVE_OVERFLOW, FLAG_MAC1_POSITIVE_OVERFLOW,
    FLAG_MAC2_NEGATIVE_OVERFLOW, FLAG_MAC2_POSITIVE_OVERFLOW, FLAG_MAC3_NEGATIVE_OVERFLOW,
    FLAG_MAC3_POSITIVE_OVERFLOW, FLAG_SX2_SATURATED, FLAG_SY2_SATURATED, FLAG_SZ3_OTZ_SATURATED,
};
use crate::unr::divide_unr;

/// 44-bit signed MAC maximum (`2^43 - 1`).
const MAC44_MAX: i64 = 0x7FF_FFFF_FFFF;
/// 44-bit signed MAC minimum (`-2^43`).
const MAC44_MIN: i64 = -0x800_0000_0000;

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
    mac: [i64; 3],
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
    /// MAC0 overflow bits contributed by the screen-coordinate writes of
    /// the current command (the depth-cue MAC0 write does not latch its
    /// overflow into `FLAG`; see `op_rtps_vertex`).
    mac0_screen_overflow: u32,
}

impl Default for Gte {
    fn default() -> Self {
        Self::new()
    }
}

const fn pack_pair(lo: i16, hi: i16) -> u32 {
    (lo as u16 as u32) | ((hi as u16 as u32) << 16)
}

const fn lo16(value: u32) -> i16 {
    value as u16 as i16
}

const fn hi16(value: u32) -> i16 {
    (value >> 16) as u16 as i16
}

const fn sign_extend16(value: u32) -> u32 {
    value as u16 as i16 as i32 as u32
}

const fn sign_extend44(value: i64) -> i64 {
    (value << 20) >> 20
}

fn clamp_i64(value: i64, lo: i64, hi: i64) -> i64 {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
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
            mac0_screen_overflow: 0,
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
            15 => self.push_sxy(lo16(value) as i32, hi16(value) as i32, false),
            16 => self.sz[0] = value as u16,
            17 => self.sz[1] = value as u16,
            18 => self.sz[2] = value as u16,
            19 => self.sz[3] = value as u16,
            20 => self.rgb[0] = value,
            21 => self.rgb[1] = value,
            22 => self.rgb[2] = value,
            23 => self.res1 = value,
            24 => self.mac0 = value as i32,
            25 => self.mac[0] = i64::from(value as i32),
            26 => self.mac[1] = i64::from(value as i32),
            27 => self.mac[2] = i64::from(value as i32),
            28 => {
                self.ir[1] = (((value & 0x1F) << 7) & 0x7FFF) as i16;
                self.ir[2] = ((((value >> 5) & 0x1F) << 7) & 0x7FFF) as i16;
                self.ir[3] = ((((value >> 10) & 0x1F) << 7) & 0x7FFF) as i16;
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
                self.flag = value & 0x7FFF_F000;
                self.update_error_flag();
            }
        }
    }

    /// Reads a data register via a typed [`DataReg`] identifier.
    #[must_use]
    pub fn read_data_reg(&self, reg: DataReg) -> u32 {
        self.read_data(reg.index())
    }

    /// Writes a data register via a typed [`DataReg`] identifier.
    pub fn write_data_reg(&mut self, reg: DataReg, value: u32) {
        self.write_data(reg.index(), value);
    }

    /// Reads a control register via a typed [`ControlReg`] identifier.
    #[must_use]
    pub fn read_control_reg(&self, reg: ControlReg) -> u32 {
        self.read_control(reg.index())
    }

    /// Writes a control register via a typed [`ControlReg`] identifier.
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

    /// Returns the current `FLAG` control register (including bit 31).
    #[must_use]
    pub fn flag(&self) -> u32 {
        self.flag
    }

    /// Returns `true` if the `FLAG` bit-31 error summary is set.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.flag & FLAG_ERROR != 0
    }

    fn packed_ir_color(&self) -> u32 {
        // Each 5-bit channel is IRn >> 7 saturated to 0..=31.
        let channel = |value: i16| -> u32 { (i32::from(value) >> 7).clamp(0, 31) as u32 };
        channel(self.ir[1]) | (channel(self.ir[2]) << 5) | (channel(self.ir[3]) << 10)
    }

    fn update_error_flag(&mut self) {
        if self.flag & FLAG_ERROR_MASK != 0 {
            self.flag |= FLAG_ERROR;
        } else {
            self.flag &= !FLAG_ERROR;
        }
    }

    fn set_mac(&mut self, index: usize, value: i64) {
        match index {
            0 => {
                if value > MAC44_MAX {
                    self.flag |= FLAG_MAC1_POSITIVE_OVERFLOW;
                } else if value < MAC44_MIN {
                    self.flag |= FLAG_MAC1_NEGATIVE_OVERFLOW;
                }
                self.mac[0] = sign_extend44(value);
            }
            1 => {
                if value > MAC44_MAX {
                    self.flag |= FLAG_MAC2_POSITIVE_OVERFLOW;
                } else if value < MAC44_MIN {
                    self.flag |= FLAG_MAC2_NEGATIVE_OVERFLOW;
                }
                self.mac[1] = sign_extend44(value);
            }
            _ => {
                if value > MAC44_MAX {
                    self.flag |= FLAG_MAC3_POSITIVE_OVERFLOW;
                } else if value < MAC44_MIN {
                    self.flag |= FLAG_MAC3_NEGATIVE_OVERFLOW;
                }
                self.mac[2] = sign_extend44(value);
            }
        }
    }

    fn set_mac0(&mut self, value: i64) {
        if value > i64::from(i32::MAX) {
            self.flag |= FLAG_MAC0_POSITIVE_OVERFLOW;
        } else if value < i64::from(i32::MIN) {
            self.flag |= FLAG_MAC0_NEGATIVE_OVERFLOW;
        }
        self.mac0 = value as i32;
    }

    /// MAC0 write for the RTPS/RTPT screen coordinates: like
    /// [`Self::set_mac0`], but the overflow bits are also latched into the
    /// per-command screen-overflow shadow (the depth-cue MAC0 write that
    /// follows in each vertex does not preserve its own overflow bits
    /// across vertices, but does preserve these).
    fn set_mac0_screen(&mut self, value: i64) {
        self.set_mac0(value);
        if value > i64::from(i32::MAX) {
            self.mac0_screen_overflow |= FLAG_MAC0_POSITIVE_OVERFLOW;
        } else if value < i64::from(i32::MIN) {
            self.mac0_screen_overflow |= FLAG_MAC0_NEGATIVE_OVERFLOW;
        }
    }

    fn saturate_ir(&mut self, index: usize, value: i64, lm: bool) -> i16 {
        let lo: i64 = if lm { 0 } else { -32768 };
        let flag_bit = match index {
            1 => FLAG_IR1_SATURATED,
            2 => FLAG_IR2_SATURATED,
            _ => FLAG_IR3_SATURATED,
        };
        if value > 32767 {
            self.flag |= flag_bit;
            32767
        } else if value < lo {
            self.flag |= flag_bit;
            lo as i16
        } else {
            value as i16
        }
    }

    fn store_ir_from_mac(&mut self, _sf: bool, lm: bool) {
        // MAC registers already hold the sf-shifted accumulator; hardware
        // converts via the low 32 bits of the MAC register, saturated.
        for i in 0..3 {
            let value = i64::from(self.mac[i] as i32);
            self.ir[i + 1] = self.saturate_ir(i + 1, value, lm);
        }
    }

    /// Checks the 44-bit MAC overflow flags for `acc`, stores the
    /// sf-shifted accumulator into MAC `index` (0..=2), and returns it.
    fn set_mac_acc(&mut self, index: usize, acc: i64, sf: bool) -> i64 {
        let (pos, neg) = match index {
            0 => (FLAG_MAC1_POSITIVE_OVERFLOW, FLAG_MAC1_NEGATIVE_OVERFLOW),
            1 => (FLAG_MAC2_POSITIVE_OVERFLOW, FLAG_MAC2_NEGATIVE_OVERFLOW),
            _ => (FLAG_MAC3_POSITIVE_OVERFLOW, FLAG_MAC3_NEGATIVE_OVERFLOW),
        };
        if acc > MAC44_MAX {
            self.flag |= pos;
        } else if acc < MAC44_MIN {
            self.flag |= neg;
        }
        let stored = sign_extend44(acc) >> (if sf { 12 } else { 0 });
        self.mac[index] = stored;
        stored
    }

    fn push_sz(&mut self, value: i64) {
        let clamped = if value > 0xFFFF {
            self.flag |= FLAG_SZ3_OTZ_SATURATED;
            0xFFFF
        } else if value < 0 {
            self.flag |= FLAG_SZ3_OTZ_SATURATED;
            0
        } else {
            value as u16
        };
        self.sz[0] = self.sz[1];
        self.sz[1] = self.sz[2];
        self.sz[2] = self.sz[3];
        self.sz[3] = clamped;
    }

    fn saturate_screen(&mut self, sx: i32, sy: i32) -> (i16, i16) {
        let out_sx = if sx > 1023 {
            self.flag |= FLAG_SX2_SATURATED;
            1023
        } else if sx < -1024 {
            self.flag |= FLAG_SX2_SATURATED;
            -1024
        } else {
            sx as i16
        };
        let out_sy = if sy > 1023 {
            self.flag |= FLAG_SY2_SATURATED;
            1023
        } else if sy < -1024 {
            self.flag |= FLAG_SY2_SATURATED;
            -1024
        } else {
            sy as i16
        };
        (out_sx, out_sy)
    }

    fn push_sxy(&mut self, sx: i32, sy: i32, saturate: bool) {
        let (sx, sy) = if saturate {
            self.saturate_screen(sx, sy)
        } else {
            (sx as i16, sy as i16)
        };
        self.sxy[0] = self.sxy[1];
        self.sxy[1] = self.sxy[2];
        self.sxy[2] = (sx, sy);
    }

    fn saturate_color(&mut self, index: usize, value: i64) -> u32 {
        let flag_bit = match index {
            0 => FLAG_COLOR_R_SATURATED,
            1 => FLAG_COLOR_G_SATURATED,
            _ => FLAG_COLOR_B_SATURATED,
        };
        if value > 255 {
            self.flag |= flag_bit;
            255
        } else if value < 0 {
            self.flag |= flag_bit;
            0
        } else {
            value as u32
        }
    }

    fn push_rgb(&mut self, values: [i64; 3]) {
        let r = self.saturate_color(0, values[0]);
        let g = self.saturate_color(1, values[1]);
        let b = self.saturate_color(2, values[2]);
        let code = self.rgbc & 0xFF00_0000;
        self.rgb[0] = self.rgb[1];
        self.rgb[1] = self.rgb[2];
        self.rgb[2] = r | (g << 8) | (b << 16) | code;
    }

    /// Pushes the colour FIFO from the current MAC1..3 registers.
    ///
    /// Hardware derives each component from the low 32 bits of the MAC
    /// register shifted right by 4, saturated to `0..=255`.
    fn push_rgb_from_mac(&mut self) {
        let values = [
            i64::from(self.mac[0] as i32 >> 4),
            i64::from(self.mac[1] as i32 >> 4),
            i64::from(self.mac[2] as i32 >> 4),
        ];
        self.push_rgb(values);
    }

    fn vertex(&self, index: usize) -> [i16; 3] {
        [self.vx[index], self.vy[index], self.vz[index]]
    }

    /// Multiplies `matrix` by `vector` (plus optional translation), storing
    /// MAC1..3 (sf-shifted) and IR1..3. Returns the signed 44-bit
    /// accumulators before the sf shift (used for the RTPS depth push).
    fn multiply_matrix_by_vector(
        &mut self,
        matrix: &[[i16; 3]; 3],
        vector: [i16; 3],
        translation: Option<[i32; 3]>,
        sf: bool,
        lm: bool,
    ) -> [i64; 3] {
        let mut accs = [0i64; 3];
        for i in 0..3 {
            // Hardware accumulates in a wrapping 44-bit accumulator and
            // raises the MAC overflow flags whenever a partial sum (from
            // the wrapped accumulator plus the next product) leaves the
            // signed 44-bit range.
            let mut acc: i64 = match translation {
                Some(t) => i64::from(t[i]) << 12,
                None => 0,
            };
            for j in 0..3 {
                let raw = acc + i64::from(matrix[i][j]) * i64::from(vector[j]);
                self.check_mac_overflow(i, raw);
                acc = sign_extend44(raw);
            }
            accs[i] = acc;
            self.set_mac_acc(i, acc, sf);
        }
        self.store_ir_from_mac(sf, lm);
        accs
    }

    /// Raises the MAC1..3 overflow flags if `value` is outside the signed
    /// 44-bit range.
    fn check_mac_overflow(&mut self, index: usize, value: i64) {
        let (pos, neg) = match index {
            0 => (FLAG_MAC1_POSITIVE_OVERFLOW, FLAG_MAC1_NEGATIVE_OVERFLOW),
            1 => (FLAG_MAC2_POSITIVE_OVERFLOW, FLAG_MAC2_NEGATIVE_OVERFLOW),
            _ => (FLAG_MAC3_POSITIVE_OVERFLOW, FLAG_MAC3_NEGATIVE_OVERFLOW),
        };
        if value > MAC44_MAX {
            self.flag |= pos;
        } else if value < MAC44_MIN {
            self.flag |= neg;
        }
    }

    /// Executes one COP2 command word (the 25-bit `cop2` immediate).
    ///
    /// Invalid function codes are ignored (no register or `FLAG` change),
    /// as on hardware. After a valid command the `FLAG` bit-31 error
    /// summary is recomputed from [`FLAG_ERROR_MASK`].
    pub fn execute(&mut self, command: u32) {
        let cmd = Command::new(command);
        let Some(opcode) = cmd.opcode() else {
            return;
        };
        // Hardware clears FLAG at the start of every GTE command; error and
        // saturation bits accumulate during execution only.
        self.flag = 0;
        self.mac0_screen_overflow = 0;
        let sf = cmd.sf();
        let lm = cmd.lm();
        match opcode {
            Opcode::Rtps => self.op_rtps_vertex(0, sf, lm),
            Opcode::Rtpt => {
                for v in 0..3 {
                    self.op_rtps_vertex(v, sf, lm);
                }
            }
            Opcode::Nclip => self.op_nclip(),
            Opcode::Op => self.op_op(sf, lm),
            Opcode::Dpcs => {
                let colors = [
                    i64::from(self.rgbc & 0xFF) << 4,
                    i64::from((self.rgbc >> 8) & 0xFF) << 4,
                    i64::from((self.rgbc >> 16) & 0xFF) << 4,
                ];
                self.op_depth_cue(colors, sf, lm, true);
            }
            Opcode::Intpl => {
                let colors = [
                    i64::from(self.ir[1]),
                    i64::from(self.ir[2]),
                    i64::from(self.ir[3]),
                ];
                self.op_depth_cue(colors, sf, lm, true);
            }
            Opcode::Mvmva => self.op_mvmva(cmd),
            Opcode::Ncds => self.op_ncds(0, sf, lm),
            Opcode::Cdp => self.op_cdp(sf, lm),
            Opcode::Ncdt => {
                for v in 0..3 {
                    self.op_ncds(v, sf, lm);
                }
            }
            Opcode::Nccs => self.op_nccs(0, sf, lm),
            Opcode::Cc => self.op_cc(sf, lm),
            Opcode::Ncs => self.op_ncs(0, sf, lm),
            Opcode::Nct => {
                for v in 0..3 {
                    self.op_ncs(v, sf, lm);
                }
            }
            Opcode::Sqr => self.op_sqr(sf, lm),
            Opcode::Dcpl => {
                // DCPL: RGBC multiply of IR, then depth cueing (the same
                // tail as NCDS/CDP, without the matrix stages).
                let mut endpoints = [0i64; 3];
                let mut bases = [0i64; 3];
                for i in 0..3 {
                    let byte = i64::from((self.rgbc >> (8 * i)) & 0xFF);
                    let product = i64::from(self.ir[i + 1]) * byte;
                    endpoints[i] = (product + 255) >> 8;
                    bases[i] = product << 4;
                    self.set_mac_acc(i, product << 4, sf);
                }
                self.store_ir_from_mac(sf, lm);
                self.op_depth_cue_with_bases(endpoints, bases, sf, lm, true);
            }
            Opcode::Dpct => {
                for _ in 0..3 {
                    let fifo = self.rgb[0];
                    let colors = [
                        i64::from(fifo & 0xFF) << 4,
                        i64::from((fifo >> 8) & 0xFF) << 4,
                        i64::from((fifo >> 16) & 0xFF) << 4,
                    ];
                    self.op_depth_cue(colors, sf, lm, true);
                }
            }
            Opcode::Avsz3 => self.op_avsz(true),
            Opcode::Avsz4 => self.op_avsz(false),
            Opcode::Gpf => self.op_gpf(sf, lm, false),
            Opcode::Gpl => self.op_gpf(sf, lm, true),
            Opcode::Ncct => {
                for v in 0..3 {
                    self.op_nccs(v, sf, lm);
                }
            }
        }
        self.update_error_flag();
    }

    /// Executes a decoded [`Command`].
    pub fn execute_command(&mut self, command: Command) {
        self.execute(command.raw);
    }

    fn op_rtps_vertex(&mut self, vertex: usize, sf: bool, lm: bool) {
        let vector = self.vertex(vertex);
        let matrix = self.rt;
        let translation = self.tr;
        // In RTPS/RTPT the IR3 saturation flag comes from the MAC3>>12
        // (SZ) path, not from the IR3 saturation itself; preserve any
        // accumulated bit and recompute it below.
        let saved_ir3_flag = self.flag & FLAG_IR3_SATURATED;
        let accs = self.multiply_matrix_by_vector(&matrix, vector, Some(translation), sf, lm);
        self.flag = (self.flag & !FLAG_IR3_SATURATED) | saved_ir3_flag;
        if accs[2] >> 12 > 32767 || accs[2] >> 12 < -32768 {
            self.flag |= FLAG_IR3_SATURATED;
        }
        self.push_sz(accs[2] >> 12);
        let (quotient, overflow) = divide_unr(u32::from(self.h), u32::from(self.sz[3]));
        if overflow {
            self.flag |= FLAG_DIVIDE_OVERFLOW;
        }
        // Screen coordinates use the raw division quotient, computed
        // through MAC0; these writes latch the MAC0 overflow flags.
        let q = i64::from(quotient);
        let sum_x = i64::from(self.ofx) + i64::from(self.ir[1]) * q;
        self.set_mac0_screen(sum_x);
        let sx = (sum_x >> 16) as i32;
        let sum_y = i64::from(self.ofy) + i64::from(self.ir[2]) * q;
        self.set_mac0_screen(sum_y);
        let sy = (sum_y >> 16) as i32;
        self.push_sxy(sx, sy, true);
        // MAC0/IR0 hold the depth-cueing coefficient DQB + DQA * quotient.
        // Unlike the screen writes, this MAC0 write's overflow contribution
        // is not latched: at each vertex it replaces (rather than
        // accumulates into) the MAC0 overflow bits, keeping only the
        // screen-write contribution from earlier vertices (retail-verified
        // across RTPT). The IR0 saturation bit behaves the same way: it
        // reflects the most recent vertex's clamp.
        let depth = i64::from(self.dqb) + i64::from(self.dqa) * i64::from(quotient);
        self.mac0 = depth as i32;
        let mut mac0_bits = self.mac0_screen_overflow;
        if depth > i64::from(i32::MAX) {
            mac0_bits |= FLAG_MAC0_POSITIVE_OVERFLOW;
        } else if depth < i64::from(i32::MIN) {
            mac0_bits |= FLAG_MAC0_NEGATIVE_OVERFLOW;
        }
        self.flag =
            (self.flag & !(FLAG_MAC0_POSITIVE_OVERFLOW | FLAG_MAC0_NEGATIVE_OVERFLOW)) | mac0_bits;
        let depth_shifted = depth >> 12;
        let ir0 = depth_shifted.clamp(0, 4096);
        self.flag &= !FLAG_IR0_SATURATED;
        if ir0 != depth_shifted {
            self.flag |= FLAG_IR0_SATURATED;
        }
        self.ir[0] = ir0 as i16;
    }

    fn op_nclip(&mut self) {
        let (sx0, sy0) = (i64::from(self.sxy[0].0), i64::from(self.sxy[0].1));
        let (sx1, sy1) = (i64::from(self.sxy[1].0), i64::from(self.sxy[1].1));
        let (sx2, sy2) = (i64::from(self.sxy[2].0), i64::from(self.sxy[2].1));
        let value = sx0 * sy1 + sx1 * sy2 + sx2 * sy0 - sx0 * sy2 - sx1 * sy0 - sx2 * sy1;
        self.set_mac0(value);
    }

    fn op_op(&mut self, sf: bool, lm: bool) {
        let (ir1, ir2, ir3) = (
            i64::from(self.ir[1]),
            i64::from(self.ir[2]),
            i64::from(self.ir[3]),
        );
        // Outer product uses the rotation matrix on retail hardware.
        let r = self.rt;
        self.set_mac_acc(0, i64::from(r[1][1]) * ir3 - i64::from(r[2][2]) * ir2, sf);
        self.set_mac_acc(1, i64::from(r[2][2]) * ir1 - i64::from(r[0][0]) * ir3, sf);
        self.set_mac_acc(2, i64::from(r[0][0]) * ir2 - i64::from(r[1][1]) * ir1, sf);
        self.store_ir_from_mac(sf, lm);
    }

    fn op_mvmva(&mut self, cmd: Command) {
        let sf = cmd.sf();
        let lm = cmd.lm();
        // Multiply coefficients, one row per MAC. For the reserved matrix
        // (mx = 3) the hardware multiplier inputs are garbage: row 1 is
        // `RT13` repeated, row 2 is `RT22` repeated, and row 0 is
        // `[-(R << 4), R << 4, IR0]` from the `RGBC` red byte and the
        // current `IR0` (all verified on retail hardware).
        let coeffs: [[i64; 3]; 3] = match cmd.mx() {
            0 => self.rt.map(|row| row.map(i64::from)),
            1 => self.light.map(|row| row.map(i64::from)),
            2 => self.lcolor.map(|row| row.map(i64::from)),
            _ => {
                let r = i64::from((self.rgbc & 0xFF) as u8);
                [
                    [-(r << 4), r << 4, i64::from(self.ir[0])],
                    [i64::from(self.rt[0][2]); 3],
                    [i64::from(self.rt[1][1]); 3],
                ]
            }
        };
        let vector = match cmd.v() {
            0 => self.vertex(0),
            1 => self.vertex(1),
            2 => self.vertex(2),
            _ => [self.ir[1], self.ir[2], self.ir[3]],
        };
        let shift = if sf { 12 } else { 0 };
        for (i, row) in coeffs.iter().enumerate() {
            let mut tmp: i64 = match cmd.cv() {
                0 => i64::from(self.tr[i]) << 12,
                1 => i64::from(self.bk[i]) << 12,
                2 => i64::from(self.fc[i]) << 12,
                _ => 0,
            };
            tmp = self.accumulate_mac(i, tmp + row[0] * i64::from(vector[0]));
            if cmd.cv() == 2 {
                // Hardware bug: with cv = 2 the first product is dropped
                // from the stored MAC/IR values. Only the IR saturation
                // flag sees it (with `lm` forced off), driven by the
                // (FC << 12) + first-product accumulator; the MAC overflow
                // flags accumulate over the dropped step and the two
                // surviving products exactly as accumulated here.
                let flag_value = i64::from((tmp >> shift) as i32);
                if !(-32768..=32767).contains(&flag_value) {
                    let flag_bit = match i {
                        0 => FLAG_IR1_SATURATED,
                        1 => FLAG_IR2_SATURATED,
                        _ => FLAG_IR3_SATURATED,
                    };
                    self.flag |= flag_bit;
                }
                tmp = 0;
            }
            tmp = self.accumulate_mac(i, tmp + row[1] * i64::from(vector[1]));
            tmp = self.accumulate_mac(i, tmp + row[2] * i64::from(vector[2]));
            self.mac[i] = tmp >> shift;
        }
        self.store_ir_from_mac(sf, lm);
    }

    /// One wrapping 44-bit MAC accumulation step: raises the MAC overflow
    /// flags if `value` leaves the signed 44-bit range, then wraps it.
    fn accumulate_mac(&mut self, index: usize, value: i64) -> i64 {
        self.check_mac_overflow(index, value);
        sign_extend44(value)
    }

    fn op_ncds(&mut self, vertex: usize, sf: bool, lm: bool) {
        let vector = self.vertex(vertex);
        let light = self.light;
        self.multiply_matrix_by_vector(&light, vector, None, sf, lm);
        self.op_colour_depth(sf, lm);
    }

    fn op_cdp(&mut self, sf: bool, lm: bool) {
        // CDP has no light-matrix stage: it colour-transforms the current
        // IR vector directly, then multiplies and depth-cues.
        self.op_colour_depth(sf, lm);
    }

    /// Shared tail of `NCDS`/`CDP`/`NCDT`: colour-matrix multiply, then
    /// `RGBC` multiply, then depth cueing and a colour FIFO push.
    fn op_colour_depth(&mut self, sf: bool, lm: bool) {
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        let lcolor = self.lcolor;
        let bk = self.bk;
        self.multiply_matrix_by_vector(&lcolor, ir_vec, Some(bk), sf, lm);
        // RGBC multiply stage; its result (in ceil((IR * byte) / 256) form,
        // computed as (product + 255) >> 8) is the depth-cue endpoint, and
        // (IR * byte) << 4 is the MAC base.
        let mut endpoints = [0i64; 3];
        let mut bases = [0i64; 3];
        for i in 0..3 {
            let byte = i64::from((self.rgbc >> (8 * i)) & 0xFF);
            let product = i64::from(self.ir[i + 1]) * byte;
            endpoints[i] = (product + 255) >> 8;
            bases[i] = product << 4;
            self.set_mac_acc(i, product << 4, sf);
        }
        self.store_ir_from_mac(sf, lm);
        self.op_depth_cue_with_bases(endpoints, bases, sf, lm, true);
    }

    fn op_nccs(&mut self, vertex: usize, sf: bool, lm: bool) {
        let vector = self.vertex(vertex);
        let light = self.light;
        self.multiply_matrix_by_vector(&light, vector, None, sf, lm);
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        let lcolor = self.lcolor;
        let bk = self.bk;
        self.multiply_matrix_by_vector(&lcolor, ir_vec, Some(bk), sf, lm);
        self.multiply_by_rgbc_and_push(sf, lm);
    }

    fn op_cc(&mut self, sf: bool, lm: bool) {
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        let lcolor = self.lcolor;
        let bk = self.bk;
        self.multiply_matrix_by_vector(&lcolor, ir_vec, Some(bk), sf, lm);
        self.multiply_by_rgbc_and_push(sf, lm);
    }

    fn op_ncs(&mut self, vertex: usize, sf: bool, lm: bool) {
        let vector = self.vertex(vertex);
        let light = self.light;
        self.multiply_matrix_by_vector(&light, vector, None, sf, lm);
        let ir_vec = [self.ir[1], self.ir[2], self.ir[3]];
        let lcolor = self.lcolor;
        let bk = self.bk;
        self.multiply_matrix_by_vector(&lcolor, ir_vec, Some(bk), sf, lm);
        self.push_rgb_from_mac();
    }

    /// Final colour stage of `NCCS`/`CC`/`NCCT`/`DCPL`: multiplies the
    /// current IR1..3 by the `RGBC` colour bytes, stores MAC1..3 and
    /// IR1..3, and pushes the colour FIFO.
    fn multiply_by_rgbc_and_push(&mut self, sf: bool, lm: bool) {
        for i in 0..3 {
            let byte = i64::from((self.rgbc >> (8 * i)) & 0xFF);
            let acc = (i64::from(self.ir[i + 1]) * byte) << 4;
            self.set_mac_acc(i, acc, sf);
        }
        self.store_ir_from_mac(sf, lm);
        self.push_rgb_from_mac();
    }

    fn op_sqr(&mut self, sf: bool, lm: bool) {
        for i in 0..3 {
            let v = i64::from(self.ir[i + 1]);
            self.set_mac_acc(i, v * v, sf);
        }
        self.store_ir_from_mac(sf, lm);
    }

    /// Depth-cue stage shared by `DPCS`, `INTPL`, `DPCT`, `NCDS`, `CDP`,
    /// `NCDT`, and `DCPL`. `colors` holds the per-channel endpoints in the
    /// hardware interpolation domain (`RGBC` byte << 4, an IR value, or an
    /// `ceil((IR * byte) / 256)` colour-multiply result).
    fn op_depth_cue(&mut self, colors: [i64; 3], sf: bool, lm: bool, push: bool) {
        let bases = [colors[0] << 12, colors[1] << 12, colors[2] << 12];
        self.op_depth_cue_with_bases(colors, bases, sf, lm, push);
    }

    /// Depth-cue stage with explicit per-channel MAC bases (the value the
    /// interpolation term is added to, before the `sf` shift). For plain
    /// depth cueing the base is `endpoint << 12`; for `NCDS`/`CDP`/`NCDT`
    /// it is the `(IR * byte) << 4` colour-multiply result.
    fn op_depth_cue_with_bases(
        &mut self,
        colors: [i64; 3],
        bases: [i64; 3],
        sf: bool,
        lm: bool,
        push: bool,
    ) {
        let ir0 = i64::from(self.ir[0]);
        for i in 0..3 {
            let endpoint = colors[i];
            // The (FC<<12) - (endpoint<<12) difference is checked for
            // 44-bit MAC overflow on hardware.
            self.check_mac_overflow(i, (i64::from(self.fc[i]) << 12) - (endpoint << 12));
            let diff = if sf {
                // The FC - endpoint subtraction wraps in 32 bits on hardware.
                let wrapped = (self.fc[i]).wrapping_sub(endpoint as i32);
                clamp_i64(i64::from(wrapped), -32768, 32767)
            } else {
                let fc_mac = i64::from(((self.fc[i] as u32) << 12) as i32);
                clamp_i64(fc_mac - (endpoint << 12), -32768, 32767)
            };
            let mac_pre = bases[i] + ir0 * diff;
            self.set_mac_acc(i, mac_pre, sf);
            // Hardware also raises the IR saturation flag when the
            // FC - endpoint difference itself was clamped.
            let raw_diff = if sf {
                i64::from((self.fc[i]).wrapping_sub(endpoint as i32))
            } else {
                i64::from(((self.fc[i] as u32) << 12) as i32) - (endpoint << 12)
            };
            if raw_diff != diff {
                let flag_bit = match i {
                    0 => FLAG_IR1_SATURATED,
                    1 => FLAG_IR2_SATURATED,
                    _ => FLAG_IR3_SATURATED,
                };
                self.flag |= flag_bit;
            }
        }
        self.store_ir_from_mac(sf, lm);
        if push {
            self.push_rgb_from_mac();
        }
    }

    fn op_avsz(&mut self, three: bool) {
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
        self.set_mac0(product);
        let value = product >> 12;
        let clamped = if value > 0xFFFF {
            self.flag |= FLAG_SZ3_OTZ_SATURATED;
            0xFFFF
        } else if value < 0 {
            self.flag |= FLAG_SZ3_OTZ_SATURATED;
            0
        } else {
            value as u16
        };
        self.otz = clamped;
    }

    fn op_gpf(&mut self, sf: bool, lm: bool, with_base: bool) {
        let ir0 = i64::from(self.ir[0]);
        let ir_snapshot = self.ir;
        let shift: u32 = if sf { 12 } else { 0 };
        for i in 0..3 {
            let product = (ir0 * i64::from(ir_snapshot[i + 1])) >> shift;
            let value = if with_base {
                self.mac[i] + product
            } else {
                product
            };
            self.set_mac(i, value);
        }
        self.store_ir_from_mac(sf, lm);
        self.push_rgb_from_mac();
    }
}
