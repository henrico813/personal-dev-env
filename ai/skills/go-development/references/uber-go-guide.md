# Uber Go Style Guide — condensed reference

Source: [Uber Go Style Guide](https://github.com/uber-go/guide/blob/master/style.md).
Reviewed September 10, 2026. Derived from the guide by Uber Go contributors,
originally created by Prashant Varanasi and Simon Newton. Licensed under
Apache-2.0; the license is included in this skill.

This reference retains all source sections and their rules and qualifications.
Repeated good/bad examples, benchmark figures, and supporting explanations are
condensed. It is not a verbatim copy. Source recommendations that depend on old Go
versions are identified in this skill's Go compatibility section in `README.md`.
The original source remains available for its complete examples.

## Contents

- [Introduction](#introduction)
- [Guidelines](#guidelines)
  - [Pointers to Interfaces](#pointers-to-interfaces)
  - [Verify Interface Compliance](#verify-interface-compliance)
  - [Receivers and Interfaces](#receivers-and-interfaces)
  - [Zero-value Mutexes are Valid](#zero-value-mutexes-are-valid)
  - [Copy Slices and Maps at Boundaries](#copy-slices-and-maps-at-boundaries)
    - [Receiving Slices and Maps](#receiving-slices-and-maps)
    - [Returning Slices and Maps](#returning-slices-and-maps)
  - [Defer to Clean Up](#defer-to-clean-up)
  - [Channel Size is One or None](#channel-size-is-one-or-none)
  - [Start Enums at One](#start-enums-at-one)
  - [Use `"time"` to handle time](#use-time-to-handle-time)
    - [Use `time.Time` for instants of time](#use-timetime-for-instants-of-time)
    - [Use `time.Duration` for periods of time](#use-timeduration-for-periods-of-time)
    - [Use `time.Time` and `time.Duration` with external systems](#use-timetime-and-timeduration-with-external-systems)
  - [Errors](#errors)
    - [Error Types](#error-types)
    - [Error Wrapping](#error-wrapping)
    - [Error Naming](#error-naming)
    - [Handle Errors Once](#handle-errors-once)
  - [Handle Type Assertion Failures](#handle-type-assertion-failures)
  - [Don't Panic](#dont-panic)
  - [Use go.uber.org/atomic](#use-gouberorgatomic)
  - [Avoid Mutable Globals](#avoid-mutable-globals)
  - [Avoid Embedding Types in Public Structs](#avoid-embedding-types-in-public-structs)
  - [Avoid Using Built-In Names](#avoid-using-built-in-names)
  - [Avoid `init()`](#avoid-init)
  - [Exit in Main](#exit-in-main)
    - [Exit Once](#exit-once)
  - [Use field tags in marshaled structs](#use-field-tags-in-marshaled-structs)
  - [Don't fire-and-forget goroutines](#dont-fire-and-forget-goroutines)
    - [Wait for goroutines to exit](#wait-for-goroutines-to-exit)
    - [No goroutines in `init()`](#no-goroutines-in-init)
- [Performance](#performance)
  - [Prefer strconv over fmt](#prefer-strconv-over-fmt)
  - [Avoid repeated string-to-byte conversions](#avoid-repeated-string-to-byte-conversions)
  - [Prefer Specifying Container Capacity](#prefer-specifying-container-capacity)
    - [Specifying Map Capacity Hints](#specifying-map-capacity-hints)
    - [Specifying Slice Capacity](#specifying-slice-capacity)
- [Style](#style)
  - [Avoid overly long lines](#avoid-overly-long-lines)
  - [Be Consistent](#be-consistent)
  - [Group Similar Declarations](#group-similar-declarations)
  - [Import Group Ordering](#import-group-ordering)
  - [Package Names](#package-names)
  - [Function Names](#function-names)
  - [Import Aliasing](#import-aliasing)
  - [Function Grouping and Ordering](#function-grouping-and-ordering)
  - [Reduce Nesting](#reduce-nesting)
  - [Unnecessary Else](#unnecessary-else)
  - [Top-level Variable Declarations](#top-level-variable-declarations)
  - [Prefix Unexported Globals with _](#prefix-unexported-globals-with-_)
  - [Embedding in Structs](#embedding-in-structs)
  - [Local Variable Declarations](#local-variable-declarations)
  - [nil is a valid slice](#nil-is-a-valid-slice)
  - [Reduce Scope of Variables](#reduce-scope-of-variables)
  - [Avoid Naked Parameters](#avoid-naked-parameters)
  - [Use Raw String Literals to Avoid Escaping](#use-raw-string-literals-to-avoid-escaping)
  - [Initializing Structs](#initializing-structs)
    - [Use Field Names to Initialize Structs](#use-field-names-to-initialize-structs)
    - [Omit Zero Value Fields in Structs](#omit-zero-value-fields-in-structs)
    - [Use `var` for Zero Value Structs](#use-var-for-zero-value-structs)
    - [Initializing Struct References](#initializing-struct-references)
  - [Initializing Maps](#initializing-maps)
  - [Format Strings outside Printf](#format-strings-outside-printf)
  - [Naming Printf-style Functions](#naming-printf-style-functions)
- [Patterns](#patterns)
  - [Test Tables](#test-tables)
    - [Avoid Unnecessary Complexity in Table Tests](#avoid-unnecessary-complexity-in-table-tests)
    - [Parallel Tests](#parallel-tests)
  - [Functional Options](#functional-options)
- [Linting](#linting)
  - [Lint Runners](#lint-runners)

## Introduction

These are Uber's Go conventions, not just formatting rules. Let `gofmt` handle
formatting. The guide builds on Effective Go, Go Common Mistakes, and Go Code
Review Comments, and states an example-compatibility goal of the two most recent
minor Go releases. Its introduction mentions `golint`; its Linting section now
recommends `revive` instead. Use the latter and the repository's actual checks.
Configure editor formatting and import management with `goimports`, and run
static checks rather than relying only on manual review.

## Guidelines

### Pointers to Interfaces

Pass interfaces as values. A pointer to an interface is almost never needed.
An interface can already contain a pointer to a concrete value. To modify that
value through its methods, use an appropriate pointer receiver; do not add
another pointer around the interface.

### Verify Interface Compliance

Use compile-time assertions where an interface is part of a type's contract:
exported types promising an interface, families of implementations, or cases
where losing compliance would break callers. Use the asserted type's zero value.

```go
var _ http.Handler = (*Handler)(nil)
var _ http.Handler = LogHandler{}
```

The first checks `*Handler`; the second checks `LogHandler`. Use `nil` for
pointer, slice, or map types and an empty literal for a struct value.

### Receivers and Interfaces

Value-receiver methods can be called on values and pointers. Pointer-receiver
methods require a pointer or an addressable value. Map values are not
addressable: a `map[int]S` entry cannot call a pointer-receiver method, while a
`map[int]*S` entry can. When satisfying an interface, a type with only a pointer
receiver for a required method must be passed as `*T`, not `T`. Automatic
address-taking at an ordinary call does not change interface method sets.

### Zero-value Mutexes are Valid

Use the zero value of `sync.Mutex` or `sync.RWMutex`. A struct used through a
pointer should usually contain a non-pointer mutex field. Name the field, for
example `mu sync.Mutex`. Never embed a mutex, including in an unexported struct;
its `Lock` and `Unlock` methods are implementation details.

### Copy Slices and Maps at Boundaries

Slices and maps refer to underlying data. Receiving or returning one can allow
another caller to modify state that the type intended to own.

#### Receiving Slices and Maps

Copy a received slice or map when storing it must not permit later caller
changes to modify internal state. For a slice, allocate the required length and
use `copy`; for a map, copy its entries.

#### Returning Slices and Maps

Do not expose a mutable internal collection when the API promises a snapshot.
Copy it while holding the appropriate lock, then return the copy. Returning the
internal map after unlocking does not protect the caller's later access.

### Defer to Clean Up

Use `defer` to release acquired resources such as files and locks, so every
return path performs cleanup. Do not remove it for presumed speed: Uber reserves
that tradeoff for demonstrated, extremely small hot-path operations where the
cost matters. Keep the cleanup close to successful acquisition.

### Channel Size is One or None

Use an unbuffered channel or a buffer of one by default. A larger buffer needs a
reason: explain how its size was chosen, whether it can fill under load, and what
happens to senders when it does. Do not choose an arbitrary size to hide blocking.

### Start Enums at One

Use a named type and an `iota` constant group. Usually start at `iota + 1` so an
uninitialized zero value is not accidentally a valid operation. Starting at zero
is appropriate when zero is deliberately the useful default behavior.

### Use `"time"` to handle time

Use the `time` package rather than assumptions about fixed calendar periods.
Adding 24 hours does not necessarily mean the same local time tomorrow.

#### Use `time.Time` for instants of time

Represent timestamps with `time.Time`. Compare and manipulate them with its
methods, including `Before`, `Equal`, `Add`, and `Sub`, rather than unlabelled
integer timestamps.

#### Use `time.Duration` for periods of time

Represent elapsed periods with `time.Duration`, not ambiguous integers. Pass
`10*time.Second`, not an unexplained `10`. Use `AddDate(0, 0, 1)` for the same
local time on the next calendar day; use `Add(24*time.Hour)` for 24 elapsed hours.

#### Use `time.Time` and `time.Duration` with external systems

Use these types at system boundaries when the format or library supports them.
The guide calls out duration flags, JSON timestamps, SQL timestamps when the
driver supports them, and `gopkg.in/yaml.v2` time/duration support. These are
examples, not instructions to replace the project's dependencies.

When a duration must be numeric, use `int` or `float64` and include its unit in
the field name, such as `IntervalMillis`. JSON does not automatically give a
`time.Duration` a human-readable duration encoding. For timestamps that cannot
use `time.Time`, default to an RFC 3339 string unless another format is agreed.
The `time` package does not parse leap-second timestamps or include leap seconds
in elapsed-time calculations.

### Errors

#### Error Types

Choose errors based on whether callers must match them and whether their
messages need context:

| Caller must match? | Message | Use |
| --- | --- | --- |
| No | Static | `errors.New` |
| No | Dynamic | `fmt.Errorf` |
| Yes | Static | A top-level error value made with `errors.New` |
| Yes | Dynamic | A custom error type |

Use `errors.Is` to match error values and `errors.As` to extract error types.
An exported error value or type becomes part of the package's public API.
Propagating an existing error is a separate decision covered below.

#### Error Wrapping

Return an existing error unchanged when no useful context is missing. Otherwise,
add concise context with `fmt.Errorf`:

```go
return fmt.Errorf("get user %q: %w", id, err)
```

Use `%w` when callers should be able to inspect the underlying cause; this is
usually the default. Document and test promised wrapped error values or types,
since callers may depend on them. Use `%v` when deliberately keeping that cause
out of the error chain; this hides matching, not the error's message.

Avoid repetitive context such as `failed to` at every layer. At a logging or
external-system boundary, clearly identify the message as an error, such as by
its log level or an error field.

#### Error Naming

Prefix exported error values with `Err` and unexported error values with `err`.
Unexported errors are an exception to the underscore-prefix rule for globals.
Suffix custom error type names with `Error`.

#### Handle Errors Once

Choose the response the current layer owns: match and recover, log and degrade
gracefully, translate to a domain error, or return the error with useful context.
Do not normally both log and return the same error; that repeats reports at
multiple layers. Recover only from errors the API contract lets you recognize;
propagate unexpected errors rather than treating every failure as a known case.

### Handle Type Assertion Failures

Use the comma-ok form and handle failure instead of allowing a type mismatch to
panic:

```go
value, ok := input.(string)
if !ok {
    // Handle the unexpected type.
}
```

### Don't Panic

Return errors for production failures that callers can handle. Panic/recover is
not a normal error-handling strategy. Reserve panics for irrecoverable failures;
Uber also permits initialization failures that should abort startup, such as
`template.Must` on an invalid fixed template. In tests, use `t.Fatal` or
`t.FailNow` rather than a deliberate panic for setup failures.

### Use go.uber.org/atomic

Uber recommends `go.uber.org/atomic` to hide raw values behind typed atomic
operations, including `atomic.Bool`. Mixing ordinary reads/writes with atomic
operations can race. Keep all access through the atomic API, such as `Load` and
`Swap`. This recommendation's original comparison predates the standard
library's typed atomics; consult this skill's Go compatibility guidance before choosing a
new dependency. Do not silently present Uber's package preference as a language
requirement.

### Avoid Mutable Globals

Inject dependencies instead of changing package variables. This includes
function-valued globals such as a replaceable `time.Now`. Store a clock function
on the object that needs it and supply a test clock to that object, rather than
changing a process-wide function and restoring it after the test.

### Avoid Embedding Types in Public Structs

Use a named field and explicitly forward the desired methods instead of
embedding a shared implementation. Embedding exposes a field and promotes
methods, can leak implementation details, complicate documentation, and restrict
future changes. Embedding an interface instead of a struct does not remove the
exposed-field problem.

Adding methods to an embedded interface, removing methods from an embedded
struct, removing the embedded field, or replacing its type can break users.
Forwarding methods make the supported API explicit and leave the private
implementation easier to change. Do not expose an abstract implementation solely
to share code.

### Avoid Using Built-In Names

Do not reuse predeclared identifiers such as `error` or `string` as names for
variables or parameters. Avoid them as field names too, even where that does not
technically shadow the builtin. Choose names such as `err`, `str`, or
`errorMessage`. The language permits shadowing; do not assume compilation alone
will catch this readability problem.

### Avoid `init()`

Prefer explicit initialization or a declaration initialized by a helper. When
`init()` is genuinely needed, keep it deterministic, independent of other
initializers' side effects or ordering, free of environment/global-state
inspection or mutation, and free of filesystem, network, or system-call I/O.
Move environment-dependent setup into the program's explicit lifecycle.

The guide allows cases such as initialization that cannot be expressed in one
assignment, registration hooks for SQL drivers or encodings, and deterministic
precomputation, including serverless reuse optimizations. Libraries should not
perform surprising setup merely because they were imported.

### Exit in Main

Call `os.Exit`, `log.Fatal*`, or equivalents only in `main`. Elsewhere, return
errors. A hidden process exit makes control flow and tests harder to understand
and skips deferred cleanup.

#### Exit Once

Prefer a single exit location in `main`. Put work in a testable function that
returns an error or exit code, so that function's defers run before `main` exits.
The name `run` is not mandatory. It may receive raw arguments, parsed arguments,
or dependencies; business logic may live outside `package main`. A custom error
may carry an exit code. Preserve a single owner of process termination.

### Use field tags in marshaled structs

Add explicit field-name tags for JSON, YAML, or other tag-aware formats. The
serialized field names are an external contract, not an accidental consequence
of Go field names. Renaming a Go field should not silently change that contract.

### Don't fire-and-forget goroutines

Every goroutine must either finish predictably or accept a stop signal. In both
cases, its owner must be able to wait for completion. Uncontrolled goroutines
consume resources and can keep otherwise-unused objects alive. Uber recommends
`go.uber.org/goleak` for testing packages that can leak goroutines.

A repeating worker needs both a stop mechanism and a completion mechanism. Stop
its ticker and release its resources when it finishes; sending cancellation
alone does not establish that shutdown is complete.

#### Wait for goroutines to exit

Use `sync.WaitGroup` for multiple goroutines or a `done` channel closed by one
goroutine when it finishes. Wait with `wg.Wait()` or `<-done`. The source's
`wg.Go(...)` example requires a sufficiently new Go version; otherwise use
`Add`/`Done` correctly, with `Add` before starting the goroutine.

#### No goroutines in `init()`

Do not start goroutines in `init()`. A package needing background work should
expose an object whose creation explicitly starts it and whose `Close`, `Stop`,
or `Shutdown` signals it to stop and waits for completion. Use a wait group when
that object owns multiple goroutines.

## Performance

Apply these performance-specific rules to hot paths. The source's benchmark
figures illustrate particular examples; measure the actual workload rather than
treating those figures as universal speedups.

### Prefer strconv over fmt

For primitive-to-string or string-to-primitive conversions in a hot path, prefer
`strconv` over general formatting, such as `strconv.Itoa(n)` over `fmt.Sprint(n)`.

### Avoid repeated string-to-byte conversions

Convert an unchanged string to a byte slice once when repeated calls can reuse
it, rather than repeating `[]byte(text)` inside a hot loop.

### Prefer Specifying Container Capacity

Provide a known capacity when building a collection to reduce repeated growth
and allocations.

#### Specifying Map Capacity Hints

Use `make(map[K]V, expectedEntries)` when a useful count is known. This is a
capacity hint, not a guarantee that all required memory is allocated immediately
or that inserting that many entries will cause no allocations.

#### Specifying Slice Capacity

Use `make([]T, length, capacity)`, especially `make([]T, 0, count)` before
appending a known number of values. Capacity actually reserves backing storage;
appending within it does not require growing that backing array. Do not confuse
initial length with capacity and accidentally add leading zero-valued elements.

## Style

### Avoid overly long lines

Aim for a soft limit of 99 characters. Wrap when it improves reading, but this is
not a hard limit and code may exceed it.

### Be Consistent

Maintain consistent conventions within a codebase. When adopting these rules,
prefer package-level or larger migrations rather than mixing styles within a
package. This is a scope decision to agree on, not permission for an agent to
turn an unrelated fix into a migration.

### Group Similar Declarations

Group related imports, constants, variables, and types. Do not group unrelated
constants or types solely because they are nearby. Groups are also useful
inside functions. Exception: adjacent variable declarations should be grouped
together even if the variables are otherwise unrelated.

### Import Group Ordering

Use two import groups separated by a blank line: standard library first,
everything else second. This is `goimports`' default grouping.

### Package Names

Use short, singular, lowercase names without underscores or capitals. Choose a
name that does not require an import alias at most call sites. Avoid vague names
such as `common`, `util`, `shared`, and `lib`.

### Function Names

Use MixedCaps or mixedCaps. Test function names may use underscores to group a
subject and behavior, such as `TestMyFunction_WhatIsBeingTested`.

### Import Aliasing

Alias an import when its package name differs from the final component of its
path, including the guide's `client-go` and `/v2` examples. Otherwise avoid
aliases unless imports conflict. Do not rename both packages unnecessarily when
one alias resolves a collision.

### Function Grouping and Ordering

Group methods by receiver and arrange functions in rough call order. Put type,
constant, and variable declarations before exported functions. A constructor may
follow its type and precede its other methods. Put plain helper functions toward
the end of the file.

### Reduce Nesting

Handle errors and exceptional cases first using early returns or loop
continuations. Keep the normal path out of multiple nested blocks.

### Unnecessary Else

When choosing between a default value and an alternative, initialize the default
and change it in an `if` rather than assigning it in both branches. Do this when
the rearrangement preserves behavior and makes the result clearer.

### Top-level Variable Declarations

Use `var` at package scope. Omit a redundant explicit type when the initializer
already has exactly the desired type. Include the type when it is intentionally
different, such as an interface holding a concrete implementation.

### Prefix Unexported Globals with _

Prefix unexported package-level variables and constants with `_` to distinguish
them from locals. Unexported error values instead use the `err` prefix. This is
an Uber convention; apply repository policy and package-wide consistency rather
than silently changing a few declarations in an existing package.

### Embedding in Structs

Put embedded fields first, followed by a blank line before named fields.
Embedding must add meaningful, appropriate behavior without adverse effects for
callers. Mutexes must not be embedded, even in private types.

Do not embed merely for convenience, make construction harder, destroy a useful
zero value, expose unrelated methods or fields, expose unexported types, change
copy semantics, accidentally change the outer API or type semantics, use a
non-canonical form of the inner type, expose implementation details, let callers
control internals, or wrap behavior in a way that surprises users.

Ask whether every exported embedded method and field belongs directly on the
outer type. When only some belong, use a named field and forwarding methods.
This also applies to embedded interfaces; their fields are still exposed.

### Local Variable Declarations

Use `:=` for an explicit initial value, unless declaration grouping applies.
Use `var` when declaring an intentional zero value, such as `var filtered []int`.

### nil is a valid slice

A nil slice has length zero and supports `append`. Prefer returning `nil` instead
of a deliberately allocated empty slice when there is no contractual difference.
Check emptiness with `len(s) == 0`, not `s == nil`. Remember that nil and allocated
empty slices can behave differently during serialization; preserve that contract.

### Reduce Scope of Variables

Limit variables and constants to the scope that needs them, unless doing so
increases nesting. An `if` initializer is useful for an error needed only there;
keep a result outside the `if` when the normal path needs it afterward.
Constants belong at package scope only when shared by multiple functions/files
or required by the package's external contract.

### Avoid Naked Parameters

Make unexplained arguments readable. For example, the source annotates boolean
arguments as `true /* isLocal */`. Prefer meaningful custom types and constants
when they improve the API over a group of positional booleans and permit future
states without changing the parameter type.

### Use Raw String Literals to Avoid Escaping

Use backtick-delimited raw strings when they make quotes, backslashes, or
multiline content clearer than a heavily escaped interpreted string. Preserve
the intended contents when changing literal forms.

### Initializing Structs

#### Use Field Names to Initialize Structs

Almost always use keyed field names. The guide permits unkeyed literals in test
tables with three or fewer fields. This is permission, not a requirement to omit
names when names make a case easier to read.

#### Omit Zero Value Fields in Structs

Omit fields whose zero values add no information. Include them when they explain
intent, especially a test's expected zero result.

#### Use `var` for Zero Value Structs

For a struct value with every field left at zero, prefer `var user User` over
`user := User{}`. This rule concerns value declarations, not the pointer literal
in the next section or an interface-compliance assertion.

#### Initializing Struct References

Use `&T{}` or `&T{Field: value}` instead of `new(T)` for struct pointers.

### Initializing Maps

Use `make` for empty maps that will be written or populated programmatically;
include a capacity hint when known. Use a map literal for a fixed set of initial
entries. Distinguish an initialized map from `var m map[K]V`: the latter is nil
and panics on assignment.

### Format Strings outside Printf

When storing a fixed format string outside the call, make it a `const`, not a
variable, so static analysis can inspect it.

### Naming Printf-style Functions

Use a known Printf-style name when possible so `go vet` recognizes the call.
Otherwise, end the name with `f`, such as `Wrapf`, and configure vet's
`-printfuncs` option when needed, for example
`go vet -printfuncs=wrapf,statusf`.

## Patterns

### Test Tables

Use table-driven subtests for cases that perform the same test with different
inputs and expected outputs. Name the slice `tests`, each case `tt`, and use
`give` and `want` prefixes to distinguish inputs and expected results. Use
`t.Run` with a meaningful case identifier; the input itself is enough when it
identifies the case clearly. A table should make new cases and failures easier
to understand, not just save lines.

#### Avoid Unnecessary Complexity in Table Tests

Keep one narrow behavior per test or related table. Minimize chains of assertions
that depend on earlier assertions succeeding. Aim for fields and test logic
that apply to every row. Split tables or individual tests when cases need
substantially different setup, mock expectations, or assertions.

Do not build a table full of `shouldCallX`, `shouldCallY`, conditional mock
setup, or per-row setup functions. Similar input-driven cases may remain together
when that makes comparisons clearer. A short, straightforward success/error
branch controlled by a field such as `shouldErr` is explicitly allowed. This is
not a ban on every conditional assertion. Readability and maintainability decide.

#### Parallel Tests

Avoid capturing a reused loop variable when starting parallel subtests,
goroutines, or callbacks. The original guide warns about this but its displayed
loop omits the rebinding it describes. Use the version-appropriate rule in this
skill's Go compatibility guidance rather than copying that snippet blindly. Per-iteration
variables do not make shared maps, slices, fixtures, or dependencies independent.

### Functional Options

Use functional options for optional constructor or public-API arguments expected
to grow, particularly where the function already has three or more arguments.
Required arguments stay ordinary parameters. Defaults should work without
callers spelling out irrelevant options; add only the options being changed.

Uber's preferred implementation uses an exported `Option` interface with an
unexported `apply(*options)` method and a private configuration struct. Concrete
option types implement `apply`; functions such as `WithCache` return those
options. Start from defaults, apply the variadic options, then build the object.

Condensed from the guide's cache option example:

```go
type options struct {
    cache bool
}

type Option interface {
    apply(*options)
}

type cacheOption bool

func (c cacheOption) apply(opts *options) {
    opts.cache = bool(c)
}

func WithCache(c bool) Option {
    return cacheOption(c)
}
```

The source prefers this over opaque closures because concrete options can be
inspected, can use comparable values in tests where their types allow it, and
can implement interfaces such as `fmt.Stringer`. Do not introduce this pattern
where a simple, stable constructor is already clear.

## Linting

Consistency matters more than a particular approved list. Uber recommends at
least `errcheck` for unhandled errors, `goimports` for formatting/imports,
`revive` for style, `govet` for common mistakes, and `staticcheck` for static
analysis. It explicitly identifies `revive` as the successor to deprecated
`golint`. Run the repository's configured checks; do not assume every mentioned
binary is installed.

### Lint Runners

Uber recommends `golangci-lint` to configure and run multiple linters. Its
repository includes an example `.golangci.yml`. Use configuration compatible
with the installed runner version, and add other checks when they suit the
project. Do not replace the project's configuration with an unverified example.
