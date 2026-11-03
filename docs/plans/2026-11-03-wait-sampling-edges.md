# Qualify Z80 WAIT before wiring the Amstrad 40077

The user approved physical Amstrad WAIT and its v20 snapshot boundary in
emu198x/emu198x#2265. First qualify the existing CPU input, using the held
Zilog UM008011-0816 timing diagrams and vendored SpecIde bus states.

1. Add `crates/emu198x-zilog-z80/tests/wait_edges.rs`: short unsampled pulses,
   falling-edge samples, complete TW pairs, stable memory/I/O strobes, no early
   opcode/data retirement, refresh immunity and byte-exact paused-state replay.
   Reproduce the existing half-cycle hold and early/missing sampling first.
2. Correct only WAIT transitions in `crates/emu198x-zilog-z80/src/z80.rs`.
   Reuse existing rise/fall phase values for TW pairs where pins and internal
   operations agree. Do not add serialized fields or reorder enum variants.
   Any schema/API change needs a separate concrete approval before implementation.
3. Verify all no-WAIT bus waveforms and independent instruction corpora, as well
   as explicit asserted-WAIT replay at each affected edge. Preserve old bytes.
   Report any downstream CPC/MSX timing changes before selecting release scope.
4. Run formatting, all-target Clippy and workspace tests. Publish the qualified
   CPU through normal CI before a registry consumer depends on the corrected
   input. Keep source/fixture identities and deliberate failure controls.
5. Retain reviewed refs/evidence and remove this temporary clone and caches once
   its work is safely retained remotely. Workspace: /private/tmp/198x-z80-wait-edges;
   task evidence/cache: /private/tmp/198x-runtime-state-evidence and
   /private/tmp/198x-runtime-state-target.
