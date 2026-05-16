package main

import "fmt"

type vaultErrorCode int

const (
	vaultInvalidSelector vaultErrorCode = iota + 1
	vaultInvalidPersistedSelector
	vaultNoVaultConfigured
	vaultMainNotConfigured
	vaultWorkNotConfigured
	vaultReadConfigFailed
	vaultWriteConfigFailed
)

var vaultErrorMessages = map[vaultErrorCode]string{
	vaultInvalidSelector:         "invalid default vault %q; expected main or work",
	vaultInvalidPersistedSelector: "invalid default vault %q in ~/.config/pde/config.json",
	vaultNoVaultConfigured:       "no vault configured; run pde vault main set <path> or pde vault work set <path>",
	vaultMainNotConfigured:       "main vault not configured; run pde vault main set <path>",
	vaultWorkNotConfigured:       "work vault not configured; run pde vault work set <path>",
	vaultReadConfigFailed:        "read ~/.config/pde/config.json: %v",
	vaultWriteConfigFailed:       "write ~/.config/pde/config.json: %v",
}

type vaultError struct {
	Code    vaultErrorCode
	Message string
	Err     error
}

func (e *vaultError) Error() string { return e.Message }

func (e *vaultError) Unwrap() error { return e.Err }

func newVaultError(code vaultErrorCode, err error, args ...any) *vaultError {
	return &vaultError{Code: code, Message: fmt.Sprintf(vaultErrorMessages[code], args...), Err: err}
}
