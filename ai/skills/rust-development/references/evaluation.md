# Evaluate this skill

Use these cases when installing or revising the skill, not during every Rust
change. Run them in the intended agent and inspect both activation and output.
These are proposed acceptance cases, not evidence of completed agent tests.

## Existing everyday behavior

| Request | Expected behavior |
| --- | --- |
| Add configuration parsing to this Rust CLI. | Reads the existing API and tests; keeps parsing testable; does not invent a provider framework. |
| Fix this borrow-checker error. | Explains the ownership issue, makes a focused fix, and does not blindly add cloning or shared pointers. |
| Review this Rust module for maintainability. | Checks behavior, names, documentation, ownership, and tests; separates defects from preferences. |
| Add tests for this Rust function. | Uses production behavior, clear names, and visible inputs; chooses tables only for comparable cases. |
| Make this external device operation testable. | Checks existing boundaries, uses real deterministic parts, and substitutes the external dependency only where needed. |
| Organize this growing Rust CLI. | Groups related behavior, keeps entry-point logic small, and avoids one file per struct or trait. |

## Added coverage and reference-loading checks

| Request | Must read | Expected behavior |
| --- | --- | --- |
| Prevent user IDs and project IDs from being mixed up. | API design | Uses distinct types where useful; explains why aliases do not enforce separation. |
| Add a validated type with a default and deserialization. | API design | Checks every construction path; does not create an invalid default or bypass validation through a derive. |
| Review this type with manual equality and derived hashing. | API design | Checks that equal values hash equally; fixes a real semantic mismatch, not merely a formatting preference. |
| Make this error easier to handle. | Errors and resources | Preserves actionable failure information and underlying causes; does not flatten everything into strings. |
| Ensure buffered output errors reach the caller. | Errors and resources | Uses explicit fallible completion; does not claim drop guarantees successful flushing or persistence. |
| Change the public module layout without breaking callers. | API design | Reviews public paths, re-exports, bounds, and behavior; preserves compatibility instead of blindly enforcing one path. |
| Add an optional parser dependency. | Cargo | Checks dependency quality, MSRV, and additive features; verifies relevant default and minimal configurations. |
| Fix compilation without default features. | Cargo | Investigates feature activation; does not enable every feature to hide the broken configuration. |
| Convert this function to async while preserving its lock-protected state. | Concurrency | Checks lock scopes, blocking work, task ownership, and cancellation; does not hold a blocking guard across an await. |
| This spawned task fails a Send or static bound. | Concurrency | Explains retained values and captures; does not add unsafe marker implementations or claim static values must live forever. |
| Cancel a timed-out operation. | Concurrency | Checks whether underlying work stops, partial effects, and cleanup; does not equate stopping the wait with undoing the operation. |
| Wrap this C API in safe Rust. | Unsafe and FFI, errors and resources | Reviews ABI, pointer and ownership requirements, error paths, cleanup, and supported validation. |
| Speed this up with unchecked indexing. | Unsafe and FFI | Requires a concrete need and measurements, considers safe alternatives, and supplies a safety argument. |
| Simplify this Rust API for a beginner. | API design | Explains useful Rust concepts instead of deleting necessary types, guarantees, or error information. |

## Negative and constraint cases

| Request or condition | Expected behavior |
| --- | --- |
| Write the same feature in Go. | Does not apply this Rust skill. |
| What does `let` mean? | Answers the syntax question without inventing a repository-editing workflow. |
| Fix a typo in a Rust comment. | Makes the small change; does not load unrelated concurrency or unsafe material. |
| The repository pins an older compiler or supports an embedded target. | Keeps compatible syntax and dependencies; does not claim a host test validates the device. |
| No Rust compiler or test hardware is available. | Reports the limitation and separates inspection from executed checks. |

For each positive case, check the diff for unrequested changes and run the
project's applicable checks. Inspect whether required local references were
actually read before decisions were made. Try paraphrased requests and both
small applications and public libraries. Compare the original and revised skill
on the same cases to check that added coverage did not encourage overengineering.
Frontmatter parsing does not prove that an agent selects or follows the skill.

## Educational behavior and example use

Compare the previous and expanded skills on the same repository tasks. Inspect
the code, the explanation, and the files actually read. More text is not evidence
of better behavior.

| Task | What to look for |
| --- | --- |
| Add a small retry-count parser. | Uses a function, not a parser object or provider trait without a requirement. Explains the borrowed input and error result. |
| Add build summaries beside report summaries. | Introduces a shared interface only if callers benefit. Explains which requirement changed. |
| Display a list containing different summary types. | Explains the distinction between a concrete generic argument and a borrowed trait object. Does not box owned values unnecessarily. |
| Rename a draft without changing the original. | Explains when cloning creates independent data and why cloning a shared handle can behave differently. |
| Use a user ID where a project ID was expected. | Uses distinct types when the distinction matters. Explains why a type alias does not enforce it. |
| Fix an ignored flush error. | Makes fallible completion observable and includes a focused failure test. Does not confuse flushing with durable storage. |
| Make a table test easier to understand. | Keeps inputs and results visible; splits unrelated setup or assertions instead of creating more flags. |
| Explain a method taking self rather than a borrow. | Ties the explanation to ownership at the call site, not a detached glossary. |
| Correct a comment typo. | Makes the small correction without loading every reference or appending a Rust lesson. |
| Copy the report example into an existing no_std crate. | Adapts the idea to the project's constraints rather than copying std dependencies or the teaching layout. |

Check that explanations state a requirement, show the relevant code, explain the
choice, and mention a useful alternative when needed. They should not repeatedly
label the reader a beginner, narrate every line, or use promotional claims in
place of specifics. Source excerpts must stay brief, exact, and attributed; new
examples must not be misrepresented as copied source code.

## Validate the package and examples

Parse the main YAML, check local file and heading links, and confirm the archive
has `rust-development/SKILL.md` at its root. Check that every local reference is
reachable directly from the main skill. Keep headings useful and explanations
near the relevant code; do not delete necessary material to satisfy an arbitrary
line-count target.

The examples now form one package under `examples/`. Follow
[its README](../examples/README.md) to run formatting, Clippy, unit tests,
integration tests, and doctests. Keep quoted snippets consistent with their
source files. Clearly label explanatory fragments that are not standalone
programs. Review both the success and failure tests, not only whether the package
builds. Run the applicable unsafe tests with Miri when supported.

If a check cannot run, report it as unrun. Static inspection, frontmatter parsing,
and link validation do not establish Rust compilation or agent activation.
See the example README for this revision's recorded validation limits.

References: [Anthropic's testing guidance](https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf)
and [skill-authoring best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices).
