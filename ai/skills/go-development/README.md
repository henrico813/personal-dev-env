# Go Development Skill

This bundle provides guidance for implementing, refactoring, testing, and
reviewing Go code. `SKILL.md` defines when and how agents use it.

## Go Compatibility

Check the module's `go` directive, applicable build constraints, and supported
toolchain before applying version-sensitive examples. A newer local compiler
does not permit raising the project's minimum Go version.

### Typed Atomics

Go 1.19 added typed standard-library atomics such as `atomic.Bool` and
`atomic.Int64`. Preserve repository policy. Do not add `go.uber.org/atomic`
solely because an older guide recommends it.

See the [Go 1.19 release notes](https://go.dev/doc/go1.19#atomic).

### Range Variables

Before Go 1.22 language semantics, closures capturing a range variable may
require a per-iteration binding:

```go
for _, tt := range tests {
    tt := tt
    t.Run(tt.name, func(t *testing.T) {
        t.Parallel()
        // Use this iteration's tt.
    })
}
```

Go 1.22 gives variables declared by the loop a new instance for each iteration.
Assignment to a preexisting variable with `=` still reuses that variable.
Referenced maps, slices, fixtures, and other shared data can still race. Add
`t.Parallel()` only when cases and resources are independent.

See the [Go 1.22 release notes](https://go.dev/doc/go1.22#language).

### Wait Groups

`sync.WaitGroup.Go` requires Go 1.25. For older versions, call `Add` before
starting the goroutine, defer `Done` inside it, and call `Wait` from the owner.
Work that might not finish predictably also needs a way to stop.

See the [Go 1.25 release notes](https://go.dev/doc/go1.25#sync).

### Linters

Follow the repository's configured tools. Do not introduce `golint`, `revive`,
or another linter solely because an example names it.

## Sources And Licensing

Sources were reviewed September 10, 2026.

`references/uber-go-guide.md` is a condensed and modified adaptation of the
[Uber Go Style Guide](https://github.com/uber-go/guide/blob/master/style.md),
created by Prashant Varanasi and Simon Newton with contributions from Uber Go
contributors. The source is licensed under Apache License 2.0. The complete
license is included as `Apache-2.0.txt`.

No fixed Uber source revision is claimed. Compare the adaptation with the
moving upstream guide when updating it.

`examples/composition-original.go` reproduces the program from
[GodsBoss's June 22, 2017 Stack Overflow answer](https://stackoverflow.com/a/44707721).
`references/composition.md` reproduces selected snippets and adds explanations,
organization, a walkthrough, and production cautions.

The Stack Overflow material is provided under
[CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/) according to
[Stack Overflow's contribution licensing policy](https://stackoverflow.com/help/licensing).

## Maintenance

Update the condensed Uber guide section by section while retaining its
qualifications and exceptions. Review compatibility guidance separately so old
examples do not become new requirements. Preserve attribution, license notices,
and modification notices when sharing the bundle.
