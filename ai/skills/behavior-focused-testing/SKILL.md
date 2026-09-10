---
name: behavior-focused-testing
description: Use when writing, updating, or reviewing automated tests, including regression tests and test plans.
---

Behavior-focused testing

Protect software promises with the smallest clear, credible test suite. Follow the repository's framework and layout. Apply this workflow to the requested change, not an unsolicited suite-wide rewrite.

1. Establish the purpose

Read the requirements, production entry points, nearby tests, and repository instructions. Establish expected behavior independently of the implementation. Clarify consequential ambiguity; label tests that merely record current behavior as characterization tests.

Connect each test to the bigger picture:

Capability → Behavioral rule → Concrete scenario → Automated check

Example:

Safe firmware installation
→ Invalid images must not be written
→ Image has an invalid checksum
→ test_invalid_checksum_prevents_device_writes

Use behavior and examples to connect requirements to checks. BDD

Make the reason for the test clear from its name, visible inputs, and assertions. Add a short explanation when the reason is not obvious. Explain shared capability and architectural context once in the module or existing suite documentation, not above every test.

Prioritize high-impact hot spots: interfaces and error contracts, state transitions and invariants, data-model validation and persistence, and boundaries between systems or layers. Select relevant failure cases and boundary values rather than exhaustive combinations.

2. Test the production API without brittleness

Invoke the API actual consumers use, not private helpers. A component's supported API is sufficient; every test need not enter through the outermost CLI. Do not mock or reimplement the behavior under test. Prefer observable outputs and state over internal call sequences. Google

Choose the narrowest scope that credibly establishes the rule. Use integration tests where the risk concerns real adapters or cross-layer behavior. Identify which dependencies are real or replaced and what remains unverified. A fake does not establish real interoperability.

Avoid incidental formatting, internal object structure, test-order dependencies, shared mutable state, arbitrary sleeps, and uncontrolled time or randomness. Check interactions, ordering, and exact representations only when they are part of the externally visible behavior. Harmless internal refactors should not invalidate behavioral assertions. Google

3. Use meaningful names of seven words or fewer

Apply DAMP: Descriptive And Meaningful Phrases to names and test bodies. Google

Every test function name must contain at most seven words, including test or Test. Count snake_case and camelCase/PascalCase word boundaries; an acronym counts as one word. Do not evade the limit through obscure abbreviations or concatenation.

Default pattern: test_<behavior>_when_<condition>. Omit the condition or when when unnecessary. Preserve framework discovery rules and language casing. An existing method–scenario–outcome convention is acceptable within the limit. Microsoft pytest

test_invalid_checksum_prevents_device_writes
test_retries_when_connection_times_out
test_save_preserves_model_fields
TestSavePreservesModelFields

Name the expected behavior, not merely the input or method. Avoid vague names such as test_error and test_save. Use module or class context to avoid repetition while keeping failure reports understandable. Put additional scenario detail in the body.

4. Prefer fewer, understandable tests

Prefer one clear test with 80% coverage over six tests with 90% when the extra cases add little protection. This is a maintenance preference, not an 80% target. Add cases for distinct, important risks. Never drop critical checks or merge unrelated behaviors merely to reduce the count.

Check one coherent behavior per test, potentially with several assertions. Do not require one test per production function. Google

Separate Arrange → Act → Assert with whitespace. Keep decisive inputs and expected results visible; do not calculate expectations by duplicating production logic. Microsoft

Prefer understandable tests over code reuse. Allow duplication when it improves readability. Helpers and fixtures should hide irrelevant construction work, not the facts explaining the outcome. Google

Use tables or parametrization when cases share setup and assertion logic. Give cases meaningful IDs, such as missing-version. Split tables that accumulate case-specific branches, callbacks, or conditional assertions. Parameterization reduces repetition, not the number of scenarios. Uber pytest

5. Verify and report

Run relevant tests and inspect collection, names, and failures. For a regression, demonstrate failure before the fix and success afterward when practical. Never weaken assertions or change expectations merely to get a passing suite; establish the intended behavior first.

Check that each test would detect its intended defect, names meet the seven-word limit, and important risks remain covered. Report the protected behavior, test scope and limitations, and commands/results. State when tests could not run. Claim coverage percentages only when measured.

References

The seven-word cap and 80%/90% tradeoff are project policies, not universal rules from these guides. Consult references when clarification is needed; routine use does not require browsing.

Software Engineering at Google, Chapter 12: production APIs, behavior, brittleness, and DAMP.

Uber Go Style Guide: Test Tables: readable tables without unnecessary complexity.

Microsoft: Unit testing best practices: naming and Arrange–Act–Assert.

Dan North: Introducing BDD: connect requirements, behavior, and examples.

pytest: Good Integration Practices and parametrization: discovery, organization, and case IDs.

Skill structure and trigger descriptions follow Anthropic's skill-building guide, OpenAI's skills overview, and Claude's authoring practices.
