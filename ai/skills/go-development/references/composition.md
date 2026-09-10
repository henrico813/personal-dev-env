# Interfaces, structs, composition, and reuse

Based on [GodsBoss's June 22, 2017 Stack Overflow answer](https://stackoverflow.com/a/44707721)
to [the linked question](https://stackoverflow.com/questions/40091142/is-there-an-established-pattern-name-for-golang-code-which-seems-similar-to-a-mi).
The code excerpts and full program are reproduced from that answer. The
explanations, organization, and production cautions are added here. This adapted
reference and the reproduced example are provided under
[CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/).

## Contents

- [The idea](#the-idea)
- [1. Ask for a capability](#1-ask-for-a-capability)
- [2. Supply an implementation](#2-supply-an-implementation)
- [3. Hold a capability in a named field](#3-hold-a-capability-in-a-named-field)
- [4. Forward the method explicitly](#4-forward-the-method-explicitly)
- [5. Embed the capability](#5-embed-the-capability)
- [6. Combine capabilities and customize a call](#6-combine-capabilities-and-customize-a-call)
- [Full original program](#full-original-program)
- [What to copy into production](#what-to-copy-into-production)

## The idea

An interface describes what a value can do. A concrete type supplies the code
that does it. A struct can hold those values and combine their capabilities.
That lets different types reuse the same implementation, or use different
implementations behind the same interface.

The example has two capabilities: yelling and whispering. It combines them in a
person and a robotic voice. The useful lesson is how the pieces connect, not
whether the arrangement is called a mixin.

## 1. Ask for a capability

Original excerpt:

```go
type Yeller interface {
    Yell(message string)
}

func Yell(m Yeller, message string) {
    m.Yell(message)
}
```

`Yeller` means: "this value has a method named `Yell` that accepts a string."
The function accepts any value meeting that requirement. It does not need to know
whether the value is a person, a robot, or another implementation.

The function and method can share the name `Yell`: `Yell(value, message)` calls
the package function; `value.Yell(message)` calls a method on a value.

## 2. Supply an implementation

Original excerpt:

```go
type yeller string

func (y yeller) Yell(message string) {
    fmt.Printf(string(y), message)
}
```

`yeller` is a new named type whose underlying type is `string`. Its stored string
is a printing format, and its `Yell` method prints the message using that format.
It satisfies `Yeller` because it has the required method. Go needs no
`implements` declaration.

`yeller("%s!!!\n")` converts the string into a `yeller` value. It is not a
constructor function. `%s` is replaced by the message, and `\n` starts a new line.
The full program also supplies `twiceYeller`, which prints the message twice.

## 3. Hold a capability in a named field

Original excerpt:

```go
type Person struct {
    Yeller Yeller
}
```

The first `Yeller` is the field's name; the second is its type. A `Person` now
holds a value that can yell. That alone does not give `Person` a `Yell` method.

With this definition, the original answer distinguishes:

```go
// Won't work
Yell(person, "Loud")
```

```go
// Will work
Yell(person.Yeller, "No")
```

The first fails to compile because this version of `*Person` does not implement
`Yeller`. The second passes the field that does. It assumes the field has been
initialized to a usable implementation, such as the `yeller` in the full program.

These definitions and the next two sections are alternatives. Do not paste all
three `Person` declarations into the same package.

## 4. Forward the method explicitly

Original excerpt, added to the named-field version:

```go
func (p *Person) Yell(message string) {
    p.Yeller.Yell(message)
}

// Will work again!
Yell(person, "Yes")
```

Now `*Person` has its own `Yell` method, so it satisfies `Yeller`. That method
passes the work to the implementation stored in its field. This is forwarding.

This choice lets you expose selected operations rather than everything the
dependency can do. A production version can keep the dependency field unexported
so callers cannot reach in and replace it; the original uses an exported field
to make the example visible.

## 5. Embed the capability

Original excerpt:

```go
type Person struct {
    Yeller
}
```

There is no separate field name. This is an embedded field: it still stores a
`Yeller`, and Go also makes its `Yell` method available through `Person`.
That is method promotion. With a usable embedded value, callers can pass a
`*Person` directly to `Yell` without a handwritten forwarding method.

The field is still accessible as `person.Yeller`. Embedding therefore changes
both the methods and the fields callers can access; it is not only shorthand.

Go does not turn this into inheritance. A promoted call runs the embedded
value's method with the embedded value as its receiver. It does not secretly
pass the containing person into that implementation. See
[Effective Go: Embedding](https://go.dev/doc/effective_go#embedding).

## 6. Combine capabilities and customize a call

The full example puts both `Yeller` and `Whisperer` in each struct. The caller
chooses which implementations to store in those fields.

`Person` uses the promoted methods. `RoboticVoice` declares its own methods, so
those take precedence over the promoted ones. Its `Yell` prints a prefix,
explicitly calls `voice.Yeller.Yell(message)`, then prints a suffix. Its
`Whisper` does not call the embedded whisperer at all.

This shows two independent choices: which implementation a field holds, and
whether the containing type forwards a call unchanged or adds its own behavior.

## Full original program

The program below retains the source's code and formatting. It is also available
as `examples/composition-original.go` in this skill. Its style is preserved for
comparison with the source, not presented as an exception for new production code.

```go
package main

import (
    "fmt"
)

func main() {
    Yell(&Person{ Yeller: yeller("%s!!!\n") }, "Nooooo")
    Yell(&RoboticVoice{Yeller: twiceYeller("*** %s ***")}, "Oh no")
    Whisper(&Person{ Whisperer: whisperer("Sssssh! %s!\n")}, "...")
    Whisper(&RoboticVoice{ Whisperer: whisperer("Sssssh! %s!\n")}, "...")
}

type Yeller interface {
    Yell(message string)
}

func Yell(y Yeller, message string) {
    y.Yell(message)
}

type Whisperer interface {
    Whisper(message string)
}

func Whisper(w Whisperer, message string) {
    w.Whisper(message)
}

type Person struct {
    Yeller
    Whisperer
}

type RoboticVoice struct {
    Yeller
    Whisperer
}

func (voice *RoboticVoice) Yell(message string) {
    fmt.Printf("BEEP! ")
    voice.Yeller.Yell(message)
    fmt.Printf(" BOP!\n")
}

func (voice *RoboticVoice) Whisper(message string) {
    fmt.Printf("Error! Cannot whisper! %s\n", message)
}

type yeller string

func (y yeller) Yell(message string) {
    fmt.Printf(string(y), message)
}

type twiceYeller string

func (twice twiceYeller) Yell(message string) {
    fmt.Printf(string(twice+twice), message, message)
}

type whisperer string
func (w whisperer) Whisper(message string) {
    fmt.Printf(string(w), message)
}
```

From the skill directory, run:

```sh
go run ./examples/composition-original.go
```

Output:

```text
Nooooo!!!
BEEP! *** Oh no ****** Oh no *** BOP!
Sssssh! ...!
Error! Cannot whisper! ...
```

The four calls do the following:

| Call | Where the work goes |
| --- | --- |
| `Yell` with `Person` | The person's stored `yeller` prints the message once. |
| `Yell` with `RoboticVoice` | The robot adds its prefix/suffix around `twiceYeller`. |
| `Whisper` with `Person` | The person's stored `whisperer` prints the message. |
| `Whisper` with `RoboticVoice` | The robot's own method prints an error message and ignores the stored whisperer. |

## What to copy into production

Copy the separation of capabilities from implementations, and the deliberate
choice between a named field, forwarding, and embedding. Reuse a concrete
implementation when its behavior is genuinely shared. Add an interface where a
caller needs a capability, not automatically for every struct. Keep ordinary
functions ordinary when a struct would add no useful state or behavior.

Do not blindly copy three details from this demonstration:

**Partially initialized interfaces.** Each literal initializes only the field
used by that call. Calling a promoted method through an unset interface will
panic. Being accepted by the compiler does not guarantee every dependency is
initialized. Require valid dependencies or provide useful defaults for the
methods the object promises to support.

**An unsupported capability.** `RoboticVoice` satisfies `Whisperer` syntactically,
but printing "Cannot whisper" is not a useful implementation of a promise to
whisper. In production, do not advertise an unsupported capability, or design
its contract to return an error that callers can handle.

**Public embedded dependencies.** The source teaches how embedding works.
[Uber's public-struct guidance](https://github.com/uber-go/guide/blob/master/style.md#avoid-embedding-types-in-public-structs)
warns against exposing dependencies that way, including embedded interfaces.
For a public API, start with an unexported named field and forward the intended
methods. Embed only after checking the effects on the public fields and methods,
construction, zero value, and future changes. Saving a few lines is not enough.
