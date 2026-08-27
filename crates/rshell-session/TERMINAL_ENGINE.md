# Terminal-engine decision record

Decision: **NO-GO (unrecorded)**

The terminal-engine measurement has not been recorded against a frozen source identity. The
canary fixture deliberately contains `sha256: null`, so normal verification fails closed.

## Measurement contract

- Command: `cargo bench -p rshell-session --bench terminal_engine --locked`
- Candidate command: `cargo bench -p rshell-session --bench terminal_engine --locked -- --record-candidate`
- Selected sole adapter: `wezterm-term@d69264df66fdcc928c7a30c673df108984fda821`
- Measured implementation commit: unrecorded
- Platform and toolchain: unrecorded
- Throughput sample 1: unrecorded MiB/s
- Throughput sample 2: unrecorded MiB/s
- Throughput sample 3: unrecorded MiB/s
- Throughput sample 4: unrecorded MiB/s
- Throughput sample 5: unrecorded MiB/s
- Throughput median: unrecorded MiB/s
- 120x40 frame p95: unrecorded ms
- Scrollback digest: `sha256: null`

The executable processes exactly 104857600 bytes in each of five fresh-engine samples and
requires a median of at least 40.0 MiB/s. It measures 1000 full-dirty 120x40 frame observations
and requires nearest-rank p95 below 16.0 ms. Its scrollback oracle feeds exactly 1000 generated
labels with explicit trailing CRLF, verifies rendered rows before hashing, and canonicalizes the
verified labels with LF and no final LF.

## Recording instructions

1. Freeze the implementation in an orchestrator-authorized commit and require a clean tracked
   working tree.
2. Record `git rev-parse HEAD` as the measured implementation commit without changing that
   commit during measurement.
3. Run the candidate command. It must satisfy the unchanged thresholds, exact rendered-row
   equality, and emit every version 1 field with `decision=CANDIDATE`.
4. Copy the emitted lowercase SHA-256 into `tests/fixtures/vt/canary.json`.
5. Replace every unrecorded value above with the candidate command's source identity, platform,
   toolchain, five samples, median, p95, and digest. Change the decision only after recording
   exact evidence.
6. Run `pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1`. The normal verifier accepts
   a GO record only when the fixture, executable output, thresholds, source record, and decision
   agree exactly.
