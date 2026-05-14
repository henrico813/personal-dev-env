package main

import "fmt"

type vaultErrorCode int

const (
	vaultInvalidSelector vaultErrorCode = iota + 1
	vaultInvalidPersistedSelector
	vaultNoVaultConfigured
	vaultMainNotConfigured
	vaultWorkNotConfigured
	vaultDefaultMainRequiresPath
	vaultDefaultWorkRequiresPath
	vaultDefaultNeedsUsablePath
	vaultReadConfigFailed
	vaultWriteConfigFailed
)

var vaultErrorMessages = map[vaultErrorCode]string{
	vaultInvalidSelector:         "invalid default vault %q; expected main or work",
	vaultInvalidPersistedSelector: "invalid PDE_DEFAULT_VAULT %q; expected main or work",
	vaultNoVaultConfigured:       "no vault configured; set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env or the environment",
	vaultMainNotConfigured:       "main vault not configured",
	vaultWorkNotConfigured:       "work vault not configured",
	vaultDefaultMainRequiresPath: "default vault selector %q requires PDE_MAIN_VAULT",
	vaultDefaultWorkRequiresPath: "default vault selector %q requires PDE_WORK_VAULT",
	vaultDefaultNeedsUsablePath:  "default vault needs an existing PDE_WORK_VAULT or PDE_MAIN_VAULT",
	vaultReadConfigFailed:        "read paths.env: %v",
	vaultWriteConfigFailed:       "write paths.env: %v",
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
