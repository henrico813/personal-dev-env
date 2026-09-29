# Add a run phase

1. Add the `RunPhase` variant in `vibe/src/ledger.rs` at the point where it
   belongs in the persisted order. Keep its serialized snake-case name stable.

2. Add a `persist_phase` call in the stage that enters it. Give the call a
   short note for `vibe.log` and keep the phase call before the work it records.

3. Decide which `Status` describes each failure from that point. Preserve the
   metadata already available: before `pre_run_commit` keep it absent; after it
   is read keep it; after snapshots are read keep the snapshot SHAs.

4. Update the failure table in [internals](../reference/internals.md), including
   the status and retained metadata. The new phase name also appears in the
   `phase` field of `vibe status` JSON. Update the lifecycle explanation if the
   persistence boundary changes.

5. Add a process-level status test in `vibe/tests` using the shared fake Docker
   fixture in `tests/common`. The test should assert the JSON status, exit code,
   and metadata that the new phase can affect. Use a supported
   `VIBE_FAKE_MODE` value or add one when the new behavior needs a distinct
   observable scenario.

6. Run `make -C vibe check` and the targeted process test. Assert behavior
   rather than private call order.
