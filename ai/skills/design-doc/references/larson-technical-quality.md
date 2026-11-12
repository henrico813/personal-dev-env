# Larson's "Manage Technical Quality"

The skill borrows its review lenses and problem types from Will
Larson's chapter "Manage technical quality" in *Staff Engineer*:
<https://staffeng.com/guides/manage-technical-quality/>. His later
essay "How to create software quality",
<https://lethain.com/quality/>, adds the point about feedback below.
This page sums up, in our own words, the ideas the skill uses. Read
the sources for the full argument.

## Low quality is the normal state

Larson argues that low technical quality is what a growing system
usually has, not proof of bad engineers. Decisions that fit when they
were made fall short once the quality bar rises. Closing that gap is
routine work. So a design doc's problem story explains how the system
got here, and blames no one.

## The staircase of approaches

Larson orders the ways to raise quality from lightest to heaviest.
Start with the lightest one likely to work, and move up only when it
fails under scale. Real teams also move back down.

1. **Hot spots.** Measure where the problem gathers and fix that place
   directly, before reaching for a new process.
2. **Best practices.** Adopt a practice that research supports, one
   at a time, trying it with a few teams before everyone.
3. **Leverage points.** Interfaces, saved state and data models are
   where extra care keeps quality as the software changes. Try an
   interface against several real callers, a data model against
   several real cases, and state against its failures, consistency
   and speed.
4. **Technical vectors.** Point the whole organization's decisions
   the same way, through direct feedback, a written strategy, tooling,
   onboarding, team structure and architecture review.
5. **Measuring quality.** Write a precise definition of quality for
   this codebase, and instrument it.
6. **A quality team** that builds tools and systems for quality.
7. **A quality program** that sets goals for every team and tracks
   them.

The skill asks a design to name one of the first four as its problem
type, in the first Decisions bullet, and calls the fourth "vector
alignment". Measurement, a quality team and a quality program work
across an organization and are rarely one design's subject. Their
idea of a precise measure shows up as the after-release measure in
Building It and Tickets.

## Short term against long term

Quality has to be balanced across timeframes whose needs conflict,
such as shipping by next week against a platform that speeds up next
quarter. The skill asks each design to name that trade-off, in each
Decisions bullet's cost and in Step 5's timeframe check.

## Quality is learned in small steps

Larson says there is no winning, only learning and the chance to keep
going. Start small, iterate until it works, then add the next thing.
The skill's feedback-loop check asks for checkpoints, and Steps and
checkpoints gives each build step a check before the next.

## Feedback inside the loop creates quality

The later essay says feedback the engineer gets while writing the
code creates quality. Feedback after the engineer hands the code off
mostly measures it, and a problem found late tends to get a local fix
instead of a redesign. That is why a design doc pauses before code,
and why Tests and Samples gives the first expected results before the
code exists.
