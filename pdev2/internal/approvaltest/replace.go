package approvaltest

import "testing"

// Replace sets *variable to value until the test ends, then restores it,
// as in Replace(t, &fetchTimeout, 100*time.Millisecond).
func Replace[T any](t *testing.T, variable *T, value T) {
	t.Helper()
	old := *variable
	*variable = value
	t.Cleanup(func() { *variable = old })
}
