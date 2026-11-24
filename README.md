# libgte

Standalone pure-Rust PlayStation 1 GTE (Geometry Transformation Engine, MIPS COP2) hardware implementation.

## Features

- **Zero runtime dependencies** (`edition = "2021"`, `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, `Send + Sync`)
- **Complete bit-exact GTE**: all 32 data + 32 control registers with exact `mfc2`/`mtc2`/`cfc2`/`ctc2` sign/zero-extension, FIFO pushes (`SXYP`, `RGB2`, `SZ3`), `IRGB`/`ORGB` conversion, `LZCS`/`LZCR` leading-bit counting, and the `FLAG` bit-31 error summary
- **All 22 opcodes** (`RTPS`, `NCLIP`, `OP`, `DPCS`, `INTPL`, `MVMVA`, `NCDS`, `CDP`, `NCDT`, `NCCS`, `CC`, `NCS`, `NCT`, `SQR`, `DCPL`, `DPCT`, `AVSZ3`, `AVSZ4`, `RTPT`, `GPF`, `GPL`, `NCCT`) with exact `sf`/`lm`/`mx`/`v`/`cv` decoding, 44-bit MAC1..3 and 32-bit MAC0 overflow flags, and the hardware `MVMVA` quirks
- **Exact UNR division**: 257-entry hardware reciprocal table, `0x1FFFF` overflow quotient, and `FLAG` bit 17
- **Deterministic fixed-point arithmetic**: zero `f64`, zero heap allocations in command execution
- **Verified 1150/1150 bit-exact against real retail PS1 hardware vectors** from [`JaCzekanski/ps1-tests/gte/test-all`](https://github.com/JaCzekanski/ps1-tests) (MIT, 1150 tests); an embedded 23-vector subset runs in `cargo test`, the full suite is `GTE_TESTS_C`-gated (see below)

## Usage

```rust
use libgte::{Command, Gte, Opcode};

let mut gte = Gte::new();
gte.write_data(0, 100); // VXY0
gte.execute(Command::encode(Opcode::Rtps, true, false, 0, 0, 0).raw);
```

Run the full retail hardware suite (1150 tests) locally with:

```sh
GTE_TESTS_C=/path/to/ps1-tests/gte/test-all/tests.c cargo test --test hardware_vectors -- --nocapture
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
