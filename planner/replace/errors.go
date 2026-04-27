package replace

// ReplaceErrorCode classifies failures from PreviewFromData/Run so CLI callers
// can preserve stable error categories without guessing from strings.
type ReplaceErrorCode int

const (
	ReplaceInvalidOptionsError ReplaceErrorCode = iota + 1
	ReplaceReadSourceError
	ReplaceParseSourceError
	ReplaceDecodePatchError
	ReplaceValidateResultError
	ReplaceRenderResultError
	ReplaceFileNotFoundError
	ReplaceFileAmbiguousError
	ReplaceParseSplicedSourceError
)

// ReplaceError wraps the underlying failure with a stable category.
type ReplaceError struct {
	Code ReplaceErrorCode
	Err  error
}

func (e *ReplaceError) Error() string { return e.Err.Error() }

func (e *ReplaceError) Unwrap() error { return e.Err }

func newReplaceError(code ReplaceErrorCode, err error) error {
	if err == nil {
		return nil
	}
	return &ReplaceError{Code: code, Err: err}
}
