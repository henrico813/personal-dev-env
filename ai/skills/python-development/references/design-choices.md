# Choose the smallest useful design

Read this when choosing boundaries, functions, classes, composition, data
models, or execution techniques. The selection rules are this skill's policy;
the linked sources support specific considerations rather than the whole table.

## Responsibilities and boundaries

Ask what the code owns, what callers rely on, who owns mutable state and
resources, and which dependency direction keeps those facts visible. Add a
boundary when it contains a real rule, external effect, resource lifetime, or
variation. One implementation can justify that boundary; imagined future reuse
does not.

> A fundamental goal of Django’s stack is loose coupling and tight cohesion. The various layers of the framework shouldn’t “know” about each other unless absolutely necessary.

— [Django design philosophies, Loose coupling](https://docs.djangoproject.com/en/dev/misc/design-philosophies/#loose-coupling)

Keep invariants near the state-changing code. Avoid circular imports and
configuration read from unrelated locations. Do not hide a design cycle with a
late import before understanding why it exists.

## Separate decisions from effects

Keep policy calculations independent of network, filesystem, clock, process, or
database work when doing so makes the rules and failure paths clearer. The
top-level operation may remain a direct sequence that gathers inputs, applies
logic, and performs outputs.

> Let’s start off by splitting the code to separate the stateful parts from the logic. And our top-level function will contain almost no logic at all; it’s just an imperative series of steps: gather inputs, call our logic, apply outputs:

— Harry Percival and Bob Gregory,
[Cosmic Python Ch. 3](https://www.cosmicpython.com/book/chapter_03_abstractions.html#implementing-our-chosen-abstraction)

Do not create wrappers around every effect. A direct call through an already
small, application-owned API may be the clearest boundary.

## Choose a technique

| Technique | Use it when | Be careful when |
| --- | --- | --- |
| Function | A direct operation or transformation explains the work. | State and cleanup scatter solely to avoid an object. |
| Functional core | Decisions can consume values and return results without I/O. | Purity requires wrappers that obscure the workflow. |
| Class | State, identity, invariants, or lifetime need one owner. | It is only a namespace for a stateless function. |
| Dataclass | Field-based value behavior and generated methods fit. | Equality, hashing, ordering, or mutability need different semantics. |
| Enum | Alternatives are mutually exclusive and named. | Settings are independent and may validly combine. |
| Composition | Policies, backends, or outputs vary independently. | Objects merely forward calls and hide no useful detail. |
| Protocol | Callers accept small structural behavior from independent types. | Every concrete class receives a duplicate interface by default. |
| ABC | Runtime identity, registration, or shared implementation matters. | Structural acceptance is sufficient. |
| Inheritance | A framework requires it or the child is a true subtype. | Reuse relies on parent internals or a growing mixin tree. |
| Generator | Work can be processed incrementally. | Lazy failure or resource lifetime becomes obscure. |
| Decorator | Existing repeated plumbing is clearer at declaration time. | A normal call makes execution easier to trace. |

The functional HOWTO explains decomposition and iterator behavior; it does not
supply this table's selection rules. See the
[functional programming HOWTO](https://docs.python.org/3/howto/functional.html).

## Prefer composition for independent behavior

> Favor object composition over class inheritance.

— Brandon Rhodes,
[The Composition Over Inheritance Principle](https://python-patterns.guide/gang-of-four/composition-over-inheritance/)

Store collaborators in named fields or pass them to the operation. Forward only
behavior that belongs in the outer API. Inheritance remains appropriate for a
real subtype or a framework extension point.

## Model values and states directly

Use a frozen dataclass when replacement values should preserve prior state. Use
an enum and an explicit transition table when flags would permit impossible
combinations. Validate at trust boundaries. Do not add a workflow engine for a
small linear operation.

The [job example](../examples/src/python_skill_examples/jobs.py) returns a new
frozen value for each allowed transition.

## Keep advanced machinery exceptional

> Avoid these features in your code.

— [Google Python Style Guide, Power Features](https://google.github.io/styleguide/pyguide.html#219-power-features)

Google qualifies this for standard modules already using those features. This
skill likewise permits decorators, descriptors, metaclasses, and import hooks
when they solve a concrete problem more clearly than ordinary code. Do not add
them for novelty or to reduce a few explicit lines.
