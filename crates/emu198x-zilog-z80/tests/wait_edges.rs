//! WAIT is sampled on falling edges; a requested TW includes both clock halves.
//! Zilog UM008011-0816 figures 5-7 and 9; held primary CPU reference library.
use emu198x_zilog_z80::{
    Z80,
    z80::{IoPhase, M1Phase, MemPhase, NmiAckPhase, Phase},
};

fn at(phase: Phase) -> Z80 {
    let mut cpu = Z80::new();
    cpu.phase = phase;
    cpu.addr = 0x8000;
    cpu.mreq = true;
    cpu.rd = !matches!(phase, Phase::MemWrite(_) | Phase::IoWrite(_));
    cpu.wr = matches!(phase, Phase::IoWrite(_));
    cpu.m1 = matches!(phase, Phase::M1(_) | Phase::NmiAck(_));
    cpu
}

#[test]
fn memory_and_opcode_wait_are_sampled_on_the_falling_edge() {
    for (rise, fall, next) in [
        (
            Phase::M1(M1Phase::T2Rise),
            Phase::M1(M1Phase::T2Fall),
            Phase::M1(M1Phase::T3Rise),
        ),
        (
            Phase::MemRead(MemPhase::T2Rise),
            Phase::MemRead(MemPhase::T2Fall),
            Phase::MemRead(MemPhase::T3Rise),
        ),
        (
            Phase::MemWrite(MemPhase::T2Rise),
            Phase::MemWrite(MemPhase::T2Fall),
            Phase::MemWrite(MemPhase::T3Rise),
        ),
        (
            Phase::Contend(MemPhase::T2Rise),
            Phase::Contend(MemPhase::T2Fall),
            Phase::Contend(MemPhase::T3Rise),
        ),
        (
            Phase::NmiAck(NmiAckPhase::T2Rise),
            Phase::NmiAck(NmiAckPhase::T2Fall),
            Phase::NmiAck(NmiAckPhase::T3Rise),
        ),
    ] {
        let mut cpu = at(rise);
        let pc = cpu.regs.pc;
        cpu.wait = true;
        cpu.tick();
        assert_eq!(
            cpu.phase, fall,
            "rising-edge WAIT pulse must not stop {rise:?}"
        );
        cpu.wait = false;
        cpu.tick();
        assert_eq!(cpu.phase, next, "released before sample: no TW");
        let mut cpu = at(fall);
        cpu.wait = true;
        for _ in 0..3 {
            cpu.tick();
            assert_eq!(cpu.phase, rise, "sample inserts the first half of TW");
            assert_eq!(cpu.regs.pc, pc, "WAIT cannot retire an opcode");
            assert!(cpu.mreq);
            cpu.tick();
            assert_eq!(cpu.phase, fall, "second half of complete TW");
        }
        cpu.wait = false;
        cpu.tick();
        assert_eq!(cpu.phase, next);
    }
}

#[test]
fn io_samples_at_the_end_of_the_automatic_tw_and_repeats_whole_tw() {
    for (early, rise, fall, next) in [
        (
            Phase::IoRead(IoPhase::T2Fall),
            Phase::IoRead(IoPhase::T3Rise),
            Phase::IoRead(IoPhase::T3Fall),
            Phase::IoRead(IoPhase::T4Rise),
        ),
        (
            Phase::IoWrite(IoPhase::T2Fall),
            Phase::IoWrite(IoPhase::T3Rise),
            Phase::IoWrite(IoPhase::T3Fall),
            Phase::IoWrite(IoPhase::T4Rise),
        ),
    ] {
        let mut cpu = at(early);
        cpu.mreq = false;
        cpu.iorq = true;
        cpu.wait = true;
        cpu.tick();
        assert_eq!(
            cpu.phase, rise,
            "mandatory TW must be reached before sampling"
        );
        for _ in 0..3 {
            cpu.tick();
            assert_eq!(cpu.phase, fall);
            cpu.tick();
            assert_eq!(cpu.phase, rise, "external WAIT adds a complete TW");
            assert!(cpu.iorq && !cpu.mreq);
        }
        cpu.wait = false;
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.phase, next);
    }
}

fn drive(cpu: &mut Z80, ram: &mut [u8], output: &mut Vec<(u16, u8)>) {
    use emu198x_zilog_z80::BusOp;
    match cpu.bus_request() {
        Some(BusOp::MemRead) => cpu.data_in = ram[usize::from(cpu.addr)],
        Some(BusOp::MemWrite) => ram[usize::from(cpu.addr)] = cpu.data,
        Some(BusOp::IoRead) => cpu.data_in = 0xa5,
        Some(BusOp::IoWrite) => output.push((cpu.addr, cpu.data)),
        Some(BusOp::IntAck) | None => {}
    }
}

struct Outcome {
    ticks: usize,
    waits: usize,
    cpu: Vec<u8>,
    ram: Vec<u8>,
    ports: Vec<(u16, u8)>,
}

fn program(waits: usize) -> Outcome {
    let mut ram = vec![0; 65536];
    // LD HL,8000; LD A,(HL); LD (HL),A; LD BC,0003;
    // IN A,(C); OUT (C),A; HALT.
    let code = [
        0x21, 0, 0x80, 0x7e, 0x77, 0x01, 3, 0, 0xed, 0x78, 0xed, 0x79, 0x76,
    ];
    ram[..code.len()].copy_from_slice(&code);
    ram[0x8000] = 0x5a;
    let mut cpu = Z80::new();
    let mut output = Vec::new();
    let mut remaining = 0;
    let mut requested = 0;
    let mut checkpoints = 0;
    for ticks in 1..=1000 {
        if matches!(
            cpu.phase,
            Phase::M1(M1Phase::T1Rise)
                | Phase::MemRead(MemPhase::T1Rise)
                | Phase::MemWrite(MemPhase::T1Rise)
                | Phase::IoRead(IoPhase::T1Rise)
                | Phase::IoWrite(IoPhase::T1Rise)
        ) {
            remaining = waits;
        }
        let sample = matches!(
            cpu.phase,
            Phase::M1(M1Phase::T2Fall)
                | Phase::MemRead(MemPhase::T2Fall)
                | Phase::MemWrite(MemPhase::T2Fall)
                | Phase::IoRead(IoPhase::T3Fall)
                | Phase::IoWrite(IoPhase::T3Fall)
        );
        cpu.wait = sample && remaining > 0;
        if cpu.wait {
            remaining -= 1;
            requested += 1;
        }
        let mut replay = if waits > 0 {
            let bytes = postcard::to_allocvec(&cpu).expect("paused CPU");
            let mut restored: Z80 = postcard::from_bytes(&bytes).expect("same CPU layout");
            restored.rehydrate_walker_sequence();
            assert_eq!(postcard::to_allocvec(&restored).expect("round trip"), bytes);
            Some((restored, ram.clone(), output.clone()))
        } else {
            None
        };
        cpu.tick();
        drive(&mut cpu, &mut ram, &mut output);
        if let Some((restored, memory, ports)) = &mut replay {
            restored.tick();
            drive(restored, memory, ports);
            assert_eq!(
                postcard::to_allocvec(restored).expect("resumed edge"),
                postcard::to_allocvec(&cpu).expect("uninterrupted edge")
            );
            assert_eq!(*memory, ram);
            assert_eq!(*ports, output);
            checkpoints += 1;
        }
        if cpu.halt {
            assert_eq!(cpu.regs.a(), 0xa5);
            assert_eq!(ram[0x8000], 0x5a);
            assert_eq!(output, vec![(3, 0xa5)], "held I/O writes are accepted once");
            if waits > 0 {
                assert!(checkpoints > 100);
            }
            return Outcome {
                ticks,
                waits: requested,
                cpu: postcard::to_allocvec(&cpu).expect("final state"),
                ram,
                ports: output,
            };
        }
    }
    panic!("bounded guest did not halt");
}

#[test]
fn waits_add_whole_clocks_without_changing_guest_results_or_paused_replay() {
    let base = program(0);
    let waited = program(2);
    assert!(waited.waits > 20, "exercise every real bus cycle");
    assert_eq!(
        waited.ticks,
        base.ticks + 2 * waited.waits,
        "each accepted WAIT adds a full T-state"
    );
    assert_eq!(waited.cpu, base.cpu, "same complete final CPU state");
    assert_eq!(waited.ram, base.ram, "same memory writes");
    assert_eq!(waited.ports, base.ports, "same port writes");
}

#[test]
fn asserted_wait_cannot_delay_m1_refresh() {
    use emu198x_zilog_z80::z80::NmiAckPhase;
    for (start, expected) in [
        (
            Phase::M1(M1Phase::T3Rise),
            [
                Phase::M1(M1Phase::T3Fall),
                Phase::M1(M1Phase::T4Rise),
                Phase::M1(M1Phase::T4Fall),
            ],
        ),
        (
            Phase::NmiAck(NmiAckPhase::T3Rise),
            [
                Phase::NmiAck(NmiAckPhase::T3Fall),
                Phase::NmiAck(NmiAckPhase::T4Rise),
                Phase::NmiAck(NmiAckPhase::T4Fall),
            ],
        ),
    ] {
        let mut cpu = at(start);
        cpu.wait = true;
        for next in expected {
            cpu.tick();
            assert_eq!(cpu.phase, next, "WAIT is not sampled during refresh");
            assert!(cpu.rfsh);
        }
    }
}
