package main

import (
	"encoding/json"
	"errors"
	"fmt"
)

type PlannerErrorCode int

const (
	PlannerUsageError PlannerErrorCode = iota + 1
	PlannerReadInputError
	PlannerDecodeInputError
	PlannerValidateInputError
	PlannerRenderOutputError
	PlannerWriteOutputError
	// PlannerRuntimeError is the fallback for non-CLI errors surfacing through
	// reportError. Prefer constructing a typed error at the call site; this
	// exists so the JSON shape never silently misclassifies a runtime failure
	// as a validation failure.
	PlannerRuntimeError
)

var plannerErrorCodeNames = map[PlannerErrorCode]string{
	PlannerUsageError:         "USAGE",
	PlannerReadInputError:     "READ_INPUT",
	PlannerDecodeInputError:   "DECODE_INPUT",
	PlannerValidateInputError: "VALIDATE_INPUT",
	PlannerRenderOutputError:  "RENDER_OUTPUT",
	PlannerWriteOutputError:   "WRITE_OUTPUT",
	PlannerRuntimeError:       "RUNTIME",
}

type PlannerCLIError struct {
	Code         PlannerErrorCode
	Message      string
	Err          error
	RecoveryHint string
}

func (e *PlannerCLIError) Error() string { return e.Message }

func (e *PlannerCLIError) Unwrap() error { return e.Err }

func (e *PlannerCLIError) MarshalJSON() ([]byte, error) {
	codeName := plannerErrorCodeNames[e.Code]
	if codeName == "" {
		codeName = "UNKNOWN"
	}
	return json.Marshal(struct {
		Code         string `json:"code"`
		Message      string `json:"message"`
		RecoveryHint string `json:"recovery_hint,omitempty"`
	}{
		Code:         codeName,
		Message:      e.Message,
		RecoveryHint: e.RecoveryHint,
	})
}

var plannerErrorTemplates = map[PlannerErrorCode]string{
	PlannerReadInputError:     "read %s: %s",
	PlannerDecodeInputError:   "decode %s: %s",
	PlannerValidateInputError: "validate %s: %s",
	PlannerRenderOutputError:  "render %s: %s",
	PlannerWriteOutputError:   "write %s: %s",
	PlannerRuntimeError:       "%s: %s",
}

func newPlannerCLIError(code PlannerErrorCode, err error, subject string) *PlannerCLIError {
	if code == PlannerUsageError {
		return &PlannerCLIError{
			Code:    code,
			Message: subject,
			Err:     err,
		}
	}

	tmpl, ok := plannerErrorTemplates[code]
	if !ok {
		tmpl = "planner error: %s"
	}

	detail := ""
	if err != nil {
		detail = err.Error()
	}
	msg := fmt.Sprintf(tmpl, subject, detail)

	return &PlannerCLIError{
		Code:    code,
		Message: msg,
		Err:     err,
	}
}

func plannerExitCode(err error) int {
	var cliErr *PlannerCLIError
	if errors.As(err, &cliErr) && cliErr.Code == PlannerUsageError {
		return 2
	}
	if err != nil {
		return 1
	}
	return 0
}
