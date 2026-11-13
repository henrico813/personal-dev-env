---
type: issue
---

# Sample plan
---

## Overview
---

Adds a greeting.

```text
## Not a section
```

````markdown
```text
## Still not a section
```
`````

### Limits

Only English.

## Definition of Done
---

### Goals
- [ ] Greet in English

## Implementation
---

### 1. Add the greeting

Adds the function.

`hello.go`
> Adds Hello.

```diff
--- /dev/null
+++ b/hello.go
@@ -0,0 +1 @@
+package hello
```

### 2. Test the greeting

Adds a test.

`hello_test.go`
> Tests Hello.

```diff
--- /dev/null
+++ b/hello_test.go
@@ -0,0 +1 @@
+package hello
```

## Verification
---

### Automated Verification
- [ ] go test ./...

## Notes for agents
---

### Limits

Step 2 needs step 1.

### 3. Old step

Not part of Implementation.
