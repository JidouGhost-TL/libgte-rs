//! GTE register indices, command-word decoding, and typed register identifiers.
//!
//! The GTE exposes 32 data registers (COP2 registers 0..=31, accessed with
//! `mfc2`/`mtc2`) and 32 control registers (COP2 control registers 0..=31,
//! accessed with `cfc2`/`ctc2`). In the retail hardware test vectors the two
//! banks are concatenated: indices 0..=31 are data registers and 32..=63 are
//! control registers 0..=31.

/// Number of GTE data registers.
pub const DATA_REGISTER_COUNT: usize = 32;
/// Number of GTE control registers.
pub const CONTROL_REGISTER_COUNT: usize = 32;

/// Data register `VXY0` (vertex 0: `VX0` in bits 0..=15, `VY0` in bits 16..=31).
pub const VXY0: u8 = 0;
/// Data register `VZ0` (vertex 0 Z, sign-extended on read).
pub const VZ0: u8 = 1;
/// Data register `VXY1` (vertex 1: `VX1`, `VY1`).
pub const VXY1: u8 = 2;
/// Data register `VZ1` (vertex 1 Z, sign-extended on read).
pub const VZ1: u8 = 3;
/// Data register `VXY2` (vertex 2: `VX2`, `VY2`).
pub const VXY2: u8 = 4;
/// Data register `VZ2` (vertex 2 Z, sign-extended on read).
pub const VZ2: u8 = 5;
/// Data register `RGBC` (primary colour: R, G, B, CODE bytes).
pub const RGBC: u8 = 6;
/// Data register `OTZ` (ordering-table Z, zero-extended on read).
pub const OTZ: u8 = 7;
/// Data register `IR0` (interpolation value).
pub const IR0: u8 = 8;
/// Data register `IR1`.
pub const IR1: u8 = 9;
/// Data register `IR2`.
pub const IR2: u8 = 10;
/// Data register `IR3`.
pub const IR3: u8 = 11;
/// Data register `SXY0` (screen XY FIFO entry 0).
pub const SXY0: u8 = 12;
/// Data register `SXY1` (screen XY FIFO entry 1).
pub const SXY1: u8 = 13;
/// Data register `SXY2` (screen XY FIFO entry 2).
pub const SXY2: u8 = 14;
/// Data register `SXYP` (screen XY FIFO push mirror of `SXY2`).
pub const SXYP: u8 = 15;
/// Data register `SZ0` (screen Z FIFO entry 0).
pub const SZ0: u8 = 16;
/// Data register `SZ1` (screen Z FIFO entry 1).
pub const SZ1: u8 = 17;
/// Data register `SZ2` (screen Z FIFO entry 2).
pub const SZ2: u8 = 18;
/// Data register `SZ3` (screen Z FIFO entry 3).
pub const SZ3: u8 = 19;
/// Data register `RGB0` (colour FIFO entry 0).
pub const RGB0: u8 = 20;
/// Data register `RGB1` (colour FIFO entry 1).
pub const RGB1: u8 = 21;
/// Data register `RGB2` (colour FIFO entry 2).
pub const RGB2: u8 = 22;
/// Data register `RES1` (prohibited/reserved register, stored verbatim).
pub const RES1: u8 = 23;
/// Data register `MAC0` (32-bit multiply-accumulate register 0).
pub const MAC0: u8 = 24;
/// Data register `MAC1` (44-bit multiply-accumulate register 1, low 32 bits read).
pub const MAC1: u8 = 25;
/// Data register `MAC2` (44-bit multiply-accumulate register 2, low 32 bits read).
pub const MAC2: u8 = 26;
/// Data register `MAC3` (44-bit multiply-accumulate register 3, low 32 bits read).
pub const MAC3: u8 = 27;
/// Data register `IRGB` (IR1..IR3 packed as 5-bit colour, read/write conversion).
pub const IRGB: u8 = 28;
/// Data register `ORGB` (IR1..IR3 packed as 5-bit colour, read-only conversion).
pub const ORGB: u8 = 29;
/// Data register `LZCS` (leading-bit count source).
pub const LZCS: u8 = 30;
/// Data register `LZCR` (leading-bit count result, read-only).
pub const LZCR: u8 = 31;

/// Control register `RT11RT12` (rotation matrix elements RT11, RT12).
pub const RT11RT12: u8 = 0;
/// Control register `RT13RT21` (rotation matrix elements RT13, RT21).
pub const RT13RT21: u8 = 1;
/// Control register `RT22RT23` (rotation matrix elements RT22, RT23).
pub const RT22RT23: u8 = 2;
/// Control register `RT31RT32` (rotation matrix elements RT31, RT32).
pub const RT31RT32: u8 = 3;
/// Control register `RT33` (rotation matrix element RT33, sign-extended on read).
pub const RT33: u8 = 4;
/// Control register `TRX` (translation vector X).
pub const TRX: u8 = 5;
/// Control register `TRY` (translation vector Y).
pub const TRY: u8 = 6;
/// Control register `TRZ` (translation vector Z).
pub const TRZ: u8 = 7;
/// Control register `L11L12` (light matrix elements L11, L12).
pub const L11L12: u8 = 8;
/// Control register `L13L21` (light matrix elements L13, L21).
pub const L13L21: u8 = 9;
/// Control register `L22L23` (light matrix elements L22, L23).
pub const L22L23: u8 = 10;
/// Control register `L31L32` (light matrix elements L31, L32).
pub const L31L32: u8 = 11;
/// Control register `L33` (light matrix element L33, sign-extended on read).
pub const L33: u8 = 12;
/// Control register `RBK` (background colour red component).
pub const RBK: u8 = 13;
/// Control register `GBK` (background colour green component).
pub const GBK: u8 = 14;
/// Control register `BBK` (background colour blue component).
pub const BBK: u8 = 15;
/// Control register `LR1LR2` (light colour matrix elements LR1, LR2).
pub const LR1LR2: u8 = 16;
/// Control register `LR3LG1` (light colour matrix elements LR3, LG1).
pub const LR3LG1: u8 = 17;
/// Control register `LG2LG3` (light colour matrix elements LG2, LG3).
pub const LG2LG3: u8 = 18;
/// Control register `LB1LB2` (light colour matrix elements LB1, LB2).
pub const LB1LB2: u8 = 19;
/// Control register `LB3` (light colour matrix element LB3, sign-extended on read).
pub const LB3: u8 = 20;
/// Control register `RFC` (far colour red component).
pub const RFC: u8 = 21;
/// Control register `GFC` (far colour green component).
pub const GFC: u8 = 22;
/// Control register `BFC` (far colour blue component).
pub const BFC: u8 = 23;
/// Control register `OFX` (screen X offset).
pub const OFX: u8 = 24;
/// Control register `OFY` (screen Y offset).
pub const OFY: u8 = 25;
/// Control register `H` (projection plane distance).
pub const H: u8 = 26;
/// Control register `DQA` (depth-cue A coefficient).
pub const DQA: u8 = 27;
/// Control register `DQB` (depth-cue B coefficient).
pub const DQB: u8 = 28;
/// Control register `ZSF3` (Z average scale factor for 3 points).
pub const ZSF3: u8 = 29;
/// Control register `ZSF4` (Z average scale factor for 4 points).
pub const ZSF4: u8 = 30;
/// Control register `FLAG` (error/saturation flag register).
pub const FLAG: u8 = 31;

/// `FLAG` bit 12: `IR0` saturated.
pub const FLAG_IR0_SATURATED: u32 = 1 << 12;
/// `FLAG` bit 13: `SY2` saturated.
pub const FLAG_SY2_SATURATED: u32 = 1 << 13;
/// `FLAG` bit 14: `SX2` saturated.
pub const FLAG_SX2_SATURATED: u32 = 1 << 14;
/// `FLAG` bit 15: `MAC0` negative overflow.
pub const FLAG_MAC0_NEGATIVE_OVERFLOW: u32 = 1 << 15;
/// `FLAG` bit 16: `MAC0` positive overflow.
pub const FLAG_MAC0_POSITIVE_OVERFLOW: u32 = 1 << 16;
/// `FLAG` bit 17: division overflow.
pub const FLAG_DIVIDE_OVERFLOW: u32 = 1 << 17;
/// `FLAG` bit 18: `SZ3`/`OTZ` saturated.
pub const FLAG_SZ3_OTZ_SATURATED: u32 = 1 << 18;
/// `FLAG` bit 19: blue colour component saturated.
pub const FLAG_COLOR_B_SATURATED: u32 = 1 << 19;
/// `FLAG` bit 20: green colour component saturated.
pub const FLAG_COLOR_G_SATURATED: u32 = 1 << 20;
/// `FLAG` bit 21: red colour component saturated.
pub const FLAG_COLOR_R_SATURATED: u32 = 1 << 21;
/// `FLAG` bit 22: `IR3` saturated.
pub const FLAG_IR3_SATURATED: u32 = 1 << 22;
/// `FLAG` bit 23: `IR2` saturated.
pub const FLAG_IR2_SATURATED: u32 = 1 << 23;
/// `FLAG` bit 24: `IR1` saturated.
pub const FLAG_IR1_SATURATED: u32 = 1 << 24;
/// `FLAG` bit 25: `MAC3` negative overflow.
pub const FLAG_MAC3_NEGATIVE_OVERFLOW: u32 = 1 << 25;
/// `FLAG` bit 28: `MAC3` positive overflow.
pub const FLAG_MAC3_POSITIVE_OVERFLOW: u32 = 1 << 28;
/// `FLAG` bit 26: `MAC2` negative overflow.
pub const FLAG_MAC2_NEGATIVE_OVERFLOW: u32 = 1 << 26;
/// `FLAG` bit 29: `MAC2` positive overflow.
pub const FLAG_MAC2_POSITIVE_OVERFLOW: u32 = 1 << 29;
/// `FLAG` bit 27: `MAC1` negative overflow.
pub const FLAG_MAC1_NEGATIVE_OVERFLOW: u32 = 1 << 27;
/// `FLAG` bit 30: `MAC1` positive overflow.
pub const FLAG_MAC1_POSITIVE_OVERFLOW: u32 = 1 << 30;
/// `FLAG` bit 31: error summary bit (OR of all error bits in [`FLAG_ERROR_MASK`]).
pub const FLAG_ERROR: u32 = 1 << 31;
/// Mask of `FLAG` bits that feed the bit-31 error summary.
pub const FLAG_ERROR_MASK: u32 = 0x7F87_E000;

/// A typed GTE data-register identifier (COP2 registers 0..=31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DataReg {
    /// `VXY0` — vertex 0 XY.
    Vxy0 = 0,
    /// `VZ0` — vertex 0 Z.
    Vz0 = 1,
    /// `VXY1` — vertex 1 XY.
    Vxy1 = 2,
    /// `VZ1` — vertex 1 Z.
    Vz1 = 3,
    /// `VXY2` — vertex 2 XY.
    Vxy2 = 4,
    /// `VZ2` — vertex 2 Z.
    Vz2 = 5,
    /// `RGBC` — primary colour.
    Rgbc = 6,
    /// `OTZ` — ordering-table Z.
    Otz = 7,
    /// `IR0` — interpolation value.
    Ir0 = 8,
    /// `IR1`.
    Ir1 = 9,
    /// `IR2`.
    Ir2 = 10,
    /// `IR3`.
    Ir3 = 11,
    /// `SXY0` — screen XY FIFO 0.
    Sxy0 = 12,
    /// `SXY1` — screen XY FIFO 1.
    Sxy1 = 13,
    /// `SXY2` — screen XY FIFO 2.
    Sxy2 = 14,
    /// `SXYP` — screen XY FIFO push mirror.
    Sxyp = 15,
    /// `SZ0` — screen Z FIFO 0.
    Sz0 = 16,
    /// `SZ1` — screen Z FIFO 1.
    Sz1 = 17,
    /// `SZ2` — screen Z FIFO 2.
    Sz2 = 18,
    /// `SZ3` — screen Z FIFO 3.
    Sz3 = 19,
    /// `RGB0` — colour FIFO 0.
    Rgb0 = 20,
    /// `RGB1` — colour FIFO 1.
    Rgb1 = 21,
    /// `RGB2` — colour FIFO 2.
    Rgb2 = 22,
    /// `RES1` — reserved register.
    Res1 = 23,
    /// `MAC0`.
    Mac0 = 24,
    /// `MAC1`.
    Mac1 = 25,
    /// `MAC2`.
    Mac2 = 26,
    /// `MAC3`.
    Mac3 = 27,
    /// `IRGB` — packed IR colour (conversion).
    Irgb = 28,
    /// `ORGB` — packed output colour (conversion).
    Orgb = 29,
    /// `LZCS` — leading-bit count source.
    Lzcs = 30,
    /// `LZCR` — leading-bit count result.
    Lzcr = 31,
}

impl DataReg {
    /// Returns the raw COP2 register index (0..=31).
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// Converts a raw COP2 register index to a [`DataReg`], masking to 5 bits.
    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        match index & 31 {
            0 => Self::Vxy0,
            1 => Self::Vz0,
            2 => Self::Vxy1,
            3 => Self::Vz1,
            4 => Self::Vxy2,
            5 => Self::Vz2,
            6 => Self::Rgbc,
            7 => Self::Otz,
            8 => Self::Ir0,
            9 => Self::Ir1,
            10 => Self::Ir2,
            11 => Self::Ir3,
            12 => Self::Sxy0,
            13 => Self::Sxy1,
            14 => Self::Sxy2,
            15 => Self::Sxyp,
            16 => Self::Sz0,
            17 => Self::Sz1,
            18 => Self::Sz2,
            19 => Self::Sz3,
            20 => Self::Rgb0,
            21 => Self::Rgb1,
            22 => Self::Rgb2,
            23 => Self::Res1,
            24 => Self::Mac0,
            25 => Self::Mac1,
            26 => Self::Mac2,
            27 => Self::Mac3,
            28 => Self::Irgb,
            29 => Self::Orgb,
            30 => Self::Lzcs,
            _ => Self::Lzcr,
        }
    }
}

/// A typed GTE control-register identifier (COP2 control registers 0..=31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ControlReg {
    /// `RT11RT12`.
    Rt11Rt12 = 0,
    /// `RT13RT21`.
    Rt13Rt21 = 1,
    /// `RT22RT23`.
    Rt22Rt23 = 2,
    /// `RT31RT32`.
    Rt31Rt32 = 3,
    /// `RT33`.
    Rt33 = 4,
    /// `TRX`.
    Trx = 5,
    /// `TRY`.
    Try = 6,
    /// `TRZ`.
    Trz = 7,
    /// `L11L12`.
    L11L12 = 8,
    /// `L13L21`.
    L13L21 = 9,
    /// `L22L23`.
    L22L23 = 10,
    /// `L31L32`.
    L31L32 = 11,
    /// `L33`.
    L33 = 12,
    /// `RBK`.
    Rbk = 13,
    /// `GBK`.
    Gbk = 14,
    /// `BBK`.
    Bbk = 15,
    /// `LR1LR2`.
    Lr1Lr2 = 16,
    /// `LR3LG1`.
    Lr3Lg1 = 17,
    /// `LG2LG3`.
    Lg2Lg3 = 18,
    /// `LB1LB2`.
    Lb1Lb2 = 19,
    /// `LB3`.
    Lb3 = 20,
    /// `RFC`.
    Rfc = 21,
    /// `GFC`.
    Gfc = 22,
    /// `BFC`.
    Bfc = 23,
    /// `OFX`.
    Ofx = 24,
    /// `OFY`.
    Ofy = 25,
    /// `H`.
    H = 26,
    /// `DQA`.
    Dqa = 27,
    /// `DQB`.
    Dqb = 28,
    /// `ZSF3`.
    Zsf3 = 29,
    /// `ZSF4`.
    Zsf4 = 30,
    /// `FLAG`.
    Flag = 31,
}

impl ControlReg {
    /// Returns the raw COP2 control-register index (0..=31).
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// Converts a raw COP2 control-register index to a [`ControlReg`], masking to 5 bits.
    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        match index & 31 {
            0 => Self::Rt11Rt12,
            1 => Self::Rt13Rt21,
            2 => Self::Rt22Rt23,
            3 => Self::Rt31Rt32,
            4 => Self::Rt33,
            5 => Self::Trx,
            6 => Self::Try,
            7 => Self::Trz,
            8 => Self::L11L12,
            9 => Self::L13L21,
            10 => Self::L22L23,
            11 => Self::L31L32,
            12 => Self::L33,
            13 => Self::Rbk,
            14 => Self::Gbk,
            15 => Self::Bbk,
            16 => Self::Lr1Lr2,
            17 => Self::Lr3Lg1,
            18 => Self::Lg2Lg3,
            19 => Self::Lb1Lb2,
            20 => Self::Lb3,
            21 => Self::Rfc,
            22 => Self::Gfc,
            23 => Self::Bfc,
            24 => Self::Ofx,
            25 => Self::Ofy,
            26 => Self::H,
            27 => Self::Dqa,
            28 => Self::Dqb,
            29 => Self::Zsf3,
            30 => Self::Zsf4,
            _ => Self::Flag,
        }
    }
}

/// A GTE opcode (the low 6 bits of a COP2 command word).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    /// `RTPS` — perspective transformation (single vertex).
    Rtps = 0x01,
    /// `NCLIP` — normal clipping.
    Nclip = 0x06,
    /// `OP` — outer product.
    Op = 0x0C,
    /// `DPCS` — depth cueing (single, `RGBC` source).
    Dpcs = 0x10,
    /// `INTPL` — interpolation of a vector and the far colour vector.
    Intpl = 0x11,
    /// `MVMVA` — matrix/vector multiply and add.
    Mvmva = 0x12,
    /// `NCDS` — normal colour depth cue (single).
    Ncds = 0x13,
    /// `CDP` — colour depth cue (IR source).
    Cdp = 0x14,
    /// `NCDT` — normal colour depth cue (triple).
    Ncdt = 0x16,
    /// `NCCS` — normal colour colour (single).
    Nccs = 0x1B,
    /// `CC` — colour colour (IR source).
    Cc = 0x1C,
    /// `NCS` — normal colour (single).
    Ncs = 0x1E,
    /// `NCT` — normal colour (triple).
    Nct = 0x20,
    /// `SQR` — square of IR vector.
    Sqr = 0x28,
    /// `DCPL` — depth cue colour light.
    Dcpl = 0x29,
    /// `DPCT` — depth cueing (triple, `RGB` FIFO source).
    Dpct = 0x2A,
    /// `AVSZ3` — average of three Z values.
    Avsz3 = 0x2D,
    /// `AVSZ4` — average of four Z values.
    Avsz4 = 0x2E,
    /// `RTPT` — perspective transformation (triple).
    Rtpt = 0x30,
    /// `GPF` — general-purpose interpolation.
    Gpf = 0x3D,
    /// `GPL` — general-purpose interpolation with base.
    Gpl = 0x3E,
    /// `NCCT` — normal colour colour (triple).
    Ncct = 0x3F,
}

impl Opcode {
    /// Returns the 6-bit function code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Converts a 6-bit function code to an [`Opcode`], or `None` if invalid.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code & 0x3F {
            0x01 => Some(Self::Rtps),
            0x06 => Some(Self::Nclip),
            0x0C => Some(Self::Op),
            0x10 => Some(Self::Dpcs),
            0x11 => Some(Self::Intpl),
            0x12 => Some(Self::Mvmva),
            0x13 => Some(Self::Ncds),
            0x14 => Some(Self::Cdp),
            0x16 => Some(Self::Ncdt),
            0x1B => Some(Self::Nccs),
            0x1C => Some(Self::Cc),
            0x1E => Some(Self::Ncs),
            0x20 => Some(Self::Nct),
            0x28 => Some(Self::Sqr),
            0x29 => Some(Self::Dcpl),
            0x2A => Some(Self::Dpct),
            0x2D => Some(Self::Avsz3),
            0x2E => Some(Self::Avsz4),
            0x30 => Some(Self::Rtpt),
            0x3D => Some(Self::Gpf),
            0x3E => Some(Self::Gpl),
            0x3F => Some(Self::Ncct),
            _ => None,
        }
    }
}

/// A decoded COP2 command word (25-bit immediate field of a `cop2` instruction).
///
/// Layout: bits 0..=5 opcode, bit 10 `lm`, bits 13..=14 `cv` (translation
/// vector select, `MVMVA` only), bits 15..=16 `v` (vector select, `MVMVA`
/// only), bits 17..=18 `mx` (matrix select, `MVMVA` only), bit 19 `sf`
/// (shift fraction). All other bits are ignored by hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Command {
    /// Raw command word as passed to [`crate::Gte::execute`].
    pub raw: u32,
}

impl Command {
    /// Creates a [`Command`] from a raw command word.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Returns the 6-bit opcode field (bits 0..=5).
    #[must_use]
    pub const fn opcode_code(self) -> u8 {
        (self.raw & 0x3F) as u8
    }

    /// Returns the decoded [`Opcode`], or `None` if the function code is invalid.
    #[must_use]
    pub const fn opcode(self) -> Option<Opcode> {
        Opcode::from_code(self.opcode_code())
    }

    /// Returns the `lm` flag (bit 10): limit negative results to zero when set.
    #[must_use]
    pub const fn lm(self) -> bool {
        (self.raw >> 10) & 1 != 0
    }

    /// Returns the `sf` flag (bit 19): shift MAC values by 12 on IR store when set.
    #[must_use]
    pub const fn sf(self) -> bool {
        (self.raw >> 19) & 1 != 0
    }

    /// Returns the `mx` matrix-select field (bits 17..=18, `MVMVA` only).
    #[must_use]
    pub const fn mx(self) -> u8 {
        ((self.raw >> 17) & 3) as u8
    }

    /// Returns the `v` vector-select field (bits 15..=16, `MVMVA` only).
    #[must_use]
    pub const fn v(self) -> u8 {
        ((self.raw >> 15) & 3) as u8
    }

    /// Returns the `cv` translation-vector-select field (bits 13..=14, `MVMVA` only).
    #[must_use]
    pub const fn cv(self) -> u8 {
        ((self.raw >> 13) & 3) as u8
    }

    /// Builds a command word from decoded fields.
    ///
    /// `mx`, `v`, and `cv` are masked to 2 bits. This is a convenience for
    /// tests and emulator front-ends; hardware ignores unused fields.
    #[must_use]
    pub const fn encode(opcode: Opcode, sf: bool, lm: bool, mx: u8, v: u8, cv: u8) -> Self {
        let raw = (opcode as u32)
            | ((lm as u32) << 10)
            | (((cv & 3) as u32) << 13)
            | (((v & 3) as u32) << 15)
            | (((mx & 3) as u32) << 17)
            | ((sf as u32) << 19);
        Self { raw }
    }
}

impl From<u32> for Command {
    fn from(raw: u32) -> Self {
        Self::new(raw)
    }
}

impl From<Command> for u32 {
    fn from(command: Command) -> Self {
        command.raw
    }
}
