# Session Execution

## Why Sessions Exist

Surveil's standalone commands expose useful stages, but callers must otherwise coordinate task order, intermediate filenames, merge inputs, cleanup, and completion detection. `session run` moves that orchestration into Surveil while retaining the standalone commands for focused use.

## Lifecycle

A session has four phases:

1. Validate the repository, managed-root structure, task-file locations, names, and reserved output namespace.
2. Read, validate, gather, and research each task sequentially in sorted task-name order.
3. Merge the validated task reports and create integrity records for every artifact.
4. Write the receipt and atomically move the staged directory to `.surveil-session` without replacing an existing destination.

The staging directory is a sibling of the final directory under the managed root. This keeps the final move on one filesystem and gives callers one visibility boundary: the completed directory is absent until every required file has been written and flushed. The no-replace operation also preserves a destination created after the initial existence check.

## Receipt Role

The receipt is a machine-readable completion signal, not merely a list of filenames. Relative paths keep the session tree relocatable, while byte lengths and SHA-256 digests let a later caller verify every context, trace, report, and the evidence file. The receipt cannot include its own digest. It is written last inside staging and becomes visible with the rest of the directory.

Publication and command reporting are separate boundaries. The atomic move can succeed before writing the receipt path to stdout. In that case the command reports a nonzero I/O failure, but the published receipt remains the source of truth.

## Relationship to Standalone Commands

Session execution reuses the typed operations behind `gather`, `research`, and `merge`; it does not shell out to Surveil recursively. Existing command arguments, stdout, schemas, and errors remain available. Index creation stays separate because rebuilding a shared disposable index has different lifecycle and concurrency concerns.

## Deliberate V1 Boundaries

Sequential execution favors deterministic ordering and simple cleanup over throughput. Repository files and task documents remain live inputs. Each task also evaluates index usability when its own research begins, so the session does not freeze one source snapshot or index decision for every task. Retry policy, degraded evidence, concurrent workers, receipt signing, and Planner integration can be added later without weakening the completed-session guarantee.
