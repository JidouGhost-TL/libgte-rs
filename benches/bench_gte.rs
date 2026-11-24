//! GTE throughput benchmarks (`harness = false`, no external dependencies).
//!
//! Run with `cargo bench`. Prints ns/op and Mops/sec for `RTPT`, `MVMVA`,
//! `NCDT`, `NCLIP`, and a full-frame 10,000-triangle transform workload.

use std::hint::black_box;
use std::time::Instant;

use libgte::{Command, Gte, Opcode};

fn bench(name: &str, iterations: u64, mut body: impl FnMut(&mut Gte)) {
    let mut gte = setup();
    // Warm up.
    for _ in 0..1000 {
        body(&mut gte);
    }
    let start = Instant::now();
    for _ in 0..iterations {
        body(&mut gte);
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_secs_f64() * 1.0e9 / iterations as f64;
    let mops = iterations as f64 / elapsed.as_secs_f64() / 1.0e6;
    println!("{name:<28} {ns_per_op:>10.1} ns/op  {mops:>8.2} Mops/sec");
    black_box(gte.read_data(25));
}

fn setup() -> Gte {
    let mut gte = Gte::new();
    // Rotation matrix (identity-ish, 1.3.12) and translation.
    gte.write_control(0, 0x1000);
    gte.write_control(1, 0);
    gte.write_control(2, 0x1000);
    gte.write_control(3, 0);
    gte.write_control(4, 0x1000);
    gte.write_control(5, 0);
    gte.write_control(6, 0);
    gte.write_control(7, 100 << 12);
    // Light + colour matrices.
    gte.write_control(8, 0x1000);
    gte.write_control(10, 0x1000);
    gte.write_control(12, 0x1000);
    gte.write_control(16, 0x1000);
    gte.write_control(18, 0x1000);
    gte.write_control(20, 0x1000);
    // Projection + offsets.
    gte.write_control(24, 160 << 16);
    gte.write_control(25, 120 << 16);
    gte.write_control(26, 300);
    gte.write_data(0, (100i32 as u32) | (50u32 << 16));
    gte.write_data(1, 10);
    gte.write_data(2, (200i32 as u32) | (80u32 << 16));
    gte.write_data(3, 20);
    gte.write_data(4, (150i32 as u32) | (180u32 << 16));
    gte.write_data(5, 30);
    gte.write_data(6, 0x80FF_FFFF);
    gte
}

fn main() {
    println!("libgte benchmarks");
    bench("RTPT", 200_000, |gte| {
        gte.execute(Command::encode(Opcode::Rtpt, true, false, 0, 0, 0).raw);
    });
    bench("MVMVA (mx=0,v=0,cv=0)", 200_000, |gte| {
        gte.execute(Command::encode(Opcode::Mvmva, true, false, 0, 0, 0).raw);
    });
    bench("NCDT", 200_000, |gte| {
        gte.execute(Command::encode(Opcode::Ncdt, true, false, 0, 0, 0).raw);
    });
    bench("NCLIP", 500_000, |gte| {
        gte.execute(Command::encode(Opcode::Nclip, true, false, 0, 0, 0).raw);
    });
    // Full frame: 10,000 triangles, each RTPT + NCLIP + AVSZ3.
    let frame_iterations = 20u64;
    let triangles = 10_000u64;
    bench(
        "full-frame 10k tri (per tri)",
        frame_iterations * triangles,
        |gte| {
            gte.execute(Command::encode(Opcode::Rtpt, true, false, 0, 0, 0).raw);
            gte.execute(Command::encode(Opcode::Nclip, true, false, 0, 0, 0).raw);
            gte.execute(Command::encode(Opcode::Avsz3, true, false, 0, 0, 0).raw);
        },
    );
}
