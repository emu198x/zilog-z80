# zilog-z80

The Zilog Z80 CPU as a half-cycle signal-level state machine.

| Crate | Registry |
|---|---|
| [`emu198x-zilog-z80`](crates/emu198x-zilog-z80) | [crates.io](https://crates.io/crates/emu198x-zilog-z80) |

Part of [Emu198x](https://github.com/emu198x/emu198x), a family of
cycle-accurate retro-computing emulators. Used by its Sinclair, Amstrad, MSX, Sega and other Z80 machines, and by Play198x for `.ay` playback.

Each crate is versioned on its own and bumps only when it changes. Releases go
through release-plz: merging its release pull request tags the crate and
publishes it.

Licensed GPL-2.0-or-later.

## WAIT sampling compatibility

WAIT is sampled at T2 falling for memory/opcode cycles and at the end of
the automatic TW for I/O cycles. Each accepted request adds a complete TW;
a pulse ending before the sampling edge adds none. Refresh keeps clocking.
Feed the physical pin before each scheduled CPU edge.

This corrects the earlier rising-edge/half-cycle hold behaviour. Machines
which drive WAIT must qualify their timings and saved continuation before
adopting the change; a byte-compatible CPU layout does not establish identical
execution. The Spectrum Amstrad consumer is being qualified in
[emu198x/emu198x#2265](https://github.com/emu198x/emu198x/issues/2265).
The CPC's frozen continuation changes; its consumer needs separate timing and
save-policy qualification. The MSX workspace tests pass.

The CPU adds no fields and does not reorder phase variants. Existing TW
phases carry the complete rise/fall pair. Native replay tests save and resume
at every edge of a running memory/I/O program. No-WAIT bus waveforms and the
independent FUSE instruction corpus retain their previous results.
