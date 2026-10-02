package planpatch

import "fmt"

// Error codes reported by the patch engine. Callers branch on these constants
// instead of matching Git diagnostic prose, which is not a stable interface.
const (
	CodeBaseRequired       = "BASE_REQUIRED"
	CodeBaseUnavailable    = "BASE_UNAVAILABLE"
	CodeUnsupportedPath    = "UNSUPPORTED_PATH"
	CodePatchInvalid       = "PATCH_INVALID"
	CodePatchEnvelope      = "PATCH_ENVELOPE"
	CodePatchPathMismatch  = "PATCH_PATH_MISMATCH"
	CodePatchUnsupported   = "PATCH_UNSUPPORTED"
	CodePatchNotApplicable = "PATCH_NOT_APPLICABLE"
	CodePatchNoChange      = "PATCH_NO_CHANGE"
	CodePlanStale          = "PLAN_STALE"
	CodePlanBusy           = "PLAN_BUSY"
	CodePlanUnsupported    = "PLAN_UNSUPPORTED"
)

// Error carries a stable failure category while retaining the underlying Git
// diagnostic. Callers should branch on Code, not match prose from stderr.
type Error struct {
	Code  string
	Cause error
}

// Error reports only the cause. The code travels in Code so callers can emit it
// once as a structured field instead of repeating it inside the message.
func (e *Error) Error() string { return e.Cause.Error() }

func (e *Error) Unwrap() error { return e.Cause }

func failure(code, format string, args ...any) error {
	return &Error{Code: code, Cause: fmt.Errorf(format, args...)}
}
