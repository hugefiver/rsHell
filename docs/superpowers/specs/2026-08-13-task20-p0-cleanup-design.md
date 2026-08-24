# Task20 P0 Cleanup Evidence Design

## Scope

`--smoke-p0` must measure and report its own cleanup proof after the real GTK
application has stopped. Normal startup, normal shutdown, and
`--smoke-startup` retain their current behavior and never delete user
credentials.

## Design

`BootstrappedApplication` retains the `Arc<CredentialCoordinator>` created at
bootstrap. Its ordinary `shutdown` continues to use the existing application,
session, and repository shutdown path unchanged. A distinct P0 shutdown path
first performs application/session shutdown, then works only against the P0
temporary repository: it loads the catalog, deletes every profile that has a
credential reference through `CredentialCoordinator::apply_catalog`,
reconciles the credential journal, and verifies every captured reference is
absent through the coordinator. It measures the final session registry count
and only then shuts down the repository.

A focused root P0 cleanup module owns the typed measured evidence report and
the platform-state scan. The scan obtains only the values named by parsed
`secret_from_env`, `respond_auth`, and `paste_text_from_env` actions; it does
not log, serialize, format, or include those values in errors. Before the P0
temporary root is removed, it recursively scans regular files beneath it,
including SQLite, WAL, SHM, and any temporary state files, for those byte
sequences. Missing or empty named values fail the measurement rather than
claiming a clean scan.

The P0 runtime returns that typed report to the root contract. The cleanup and
internal vault checks consume only this measured report: actor/session zero,
temporary credential references absent, journal converged to zero, and state
files secret-free. Existing external QA JSON remains for genuinely external
SSH and real OS-vault validation evidence; it cannot fabricate root cleanup
facts.

## Error Handling and Tests

Cleanup precedence remains unchanged: a P0 cleanup failure overrides a GUI
failure. The P0 cleanup path returns a failed measurement instead of panicking
when a coordinator mutation, reconcile, ref lookup, scan, or repository
shutdown fails. Tests cover secret environment extraction, measured profile and
reference cleanup, journal convergence, state-file scanning, and contract
failure without measured evidence.
