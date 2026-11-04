# Code a junior engineer can understand

Prefer clear steps, descriptive intermediate values, and ordinary control flow.
Use small functions with a clear purpose, but do not split a readable operation
into helpers that force the reader to jump between files. Simple iterator chains
are welcome; use a loop when it makes branching or state changes easier to follow.

When introducing an unfamiliar Rust concept, briefly explain what it does and
why this code needs it. Pair advice against unnecessary complexity with the
conditions that would justify it. For example, a custom trait needs a reason
such as interchangeable behavior or an extension point. It does not need a rule
that every struct must have a matching trait. Describe ownership in terms of who
keeps a value and who borrows it. Put lasting constraints in the code's
documentation; put introductory syntax explanations in the handoff, not comments on every line. Do not avoid
useful Rust idioms merely because they are unfamiliar.

Explain the requirement before the technique. Show a small example, explain why
it fits, and name a useful alternative when the requirement could differ. For
instance, show a concrete function before explaining when a trait helps. Do not
call an alternative “bad” when it is simply unnecessary for this task. Keep
explanations near the code they explain, rather than collecting unexplained terms
in a glossary at the end. Avoid filler such as “leverage,” “robust solution,” and
“seamless integration.” Say what the code does and what it guarantees.
