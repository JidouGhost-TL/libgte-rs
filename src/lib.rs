//! Standalone pure-Rust PlayStation 1 GTE hardware implementation.
//!
//! `libgte` implements the PlayStation 1 Geometry Transformation Engine
//! (GTE, MIPS COP2, `CXD8530BQ` / `CXD8606BQ`) with zero runtime
//! dependencies, `#![forbid(unsafe_code)]`, deterministic fixed-point
//! integer arithmetic, and zero heap allocations in command execution.
//!
//! # Architecture
//! - [`Gte`]: the full register file and command executor. Load registers
//!   with `write_data`/`write_control` (`mtc2`/`ctc2`), run one COP2
//!   command word with [`Gte::execute`], and read results with
//!   `read_data`/`read_control` (`mfc2`/`cfc2`).
//! - [`registers`]: register index constants, typed [`registers::DataReg`]
//!   / [`registers::ControlReg`] identifiers, the [`registers::Opcode`]
//!   enum, and [`registers::Command`] command-word decoding
//!   (`sf`/`lm`/`mx`/`v`/`cv`).
//! - [`unr`]: the exact 257-entry hardware reciprocal table and UNR
//!   division with overflow behaviour.
//!
//! # Example
//! ```
//! use libgte::{Command, Gte, Opcode};
//!
//! let mut gte = Gte::new();
//! // Identity rotation matrix (1.3.12 fixed point: 0x1000 == 1.0).
//! gte.write_control(0, 0x1000); // RT11=0x1000, RT12=0
//! gte.write_control(1, 0); // RT13=0, RT21=0
//! gte.write_control(2, 0x1000); // RT22=0x1000, RT23=0
//! gte.write_control(3, 0); // RT31=0, RT32=0
//! gte.write_control(4, 0x1000); // RT33=0x1000
//! gte.write_data(0, 100); // VXY0: VX0=100, VY0=0
//! gte.write_data(1, 50); // VZ0=50
//! gte.execute(Command::encode(Opcode::Rtps, true, false, 0, 0, 0).raw);
//! assert_eq!(gte.read_data(25), 100); // MAC1 = (0x1000 * 100) >> 12
//! assert_eq!(gte.read_data(27), 50); // MAC3 = (0x1000 * 50) >> 12
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod gte;
pub mod registers;
pub mod unr;

pub use gte::Gte;
pub use registers::{Command, ControlReg, DataReg, Opcode};
pub use unr::{divide_unr, UNR_TABLE};

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn public_types_are_send_and_sync() {
        assert_send_sync::<Gte>();
        assert_send_sync::<Command>();
        assert_send_sync::<DataReg>();
        assert_send_sync::<ControlReg>();
        assert_send_sync::<Opcode>();
    }

    #[test]
    fn reset_clears_registers() {
        let mut gte = Gte::new();
        gte.write_data(0, 0xDEAD_BEEF);
        gte.write_control(31, 0xFFFF_FFFF);
        gte.reset();
        assert_eq!(gte.data_registers(), [0; 32]);
        assert_eq!(gte.control_registers(), [0; 32]);
    }
}
