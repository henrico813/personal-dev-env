package main

import "fmt"

type vaultErrorCode int

const (
	vaultInvalidSelector vaultErrorCode = iota + 1
	vaultInvalidPersistedSelector
	vaultDefaultNotConfigured
	vaultNoVaultConfigured
	vaultMainNotConfigured
	vaultWorkNotConfigured
	vaultReadConfigFailed
	vaultWriteConfigFailed
)

func vaultErrorMessage(code vaultErrorCode) string {
	switch code {
	case vaultInvalidSelector:
		return "invalid default vault %q; expected main or work"
	case vaultInvalidPersistedSelector:
		return "invalid default vault %q in ~/.config/pde/config.json"
	case vaultDefaultNotConfigured:
		return "default vault not configured; run pde vault default set <main|work>"
	case vaultNoVaultConfigured:
		return "no vault configured; run pde vault main set <path> or pde vault work set <path>"
	case vaultMainNotConfigured:
		return "main vault not configured; run pde vault main set <path>"
	case vaultWorkNotConfigured:
		return "work vault not configured; run pde vault work set <path>"
	case vaultReadConfigFailed:
		return "read ~/.config/pde/config.json: %v"
	case vaultWriteConfigFailed:
		return "write ~/.config/pde/config.json: %v"
	default:
		return "unknown vault error"
	}
}

type vaultError struct {
	Code    vaultErrorCode
	Message string
	Err     error
}

func (e *vaultError) Error() string { return e.Message }

func (e *vaultError) Unwrap() error { return e.Err }

func newVaultError(code vaultErrorCode, err error, args ...any) *vaultError {
	return &vaultError{Code: code, Message: fmt.Sprintf(vaultErrorMessage(code), args...), Err: err}
}
