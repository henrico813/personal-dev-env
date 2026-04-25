package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
)

type jsonErrResponse struct {
	Error string `json:"error"`
}

type PlannerErrorCode int

const (
	PlannerUsageError PlannerErrorCode = iota + 1
	PlannerReadInputError
	PlannerDecodeInputError
	PlannerValidateInputError
	PlannerRenderOutputError
	PlannerWriteOutputError
)

type PlannerCLIError struct {
	Code    PlannerErrorCode
	Message string
	Err     error
}

func (e *PlannerCLIError) Error() string { return e.Message }

func (e *PlannerCLIError) Unwrap() error { return e.Err }

var plannerErrorTemplates = map[PlannerErrorCode]string{
	PlannerReadInputError:     "read %s: %s",
	PlannerDecodeInputError:   "decode %s: %s",
	PlannerValidateInputError: "validate %s: %s",
	PlannerRenderOutputError:  "render %s: %s",
	PlannerWriteOutputError:   "write %s: %s",
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

func writeErr(w io.Writer, jsonErrors bool, msg string) {
	if jsonErrors {
		b, _ := json.Marshal(jsonErrResponse{Error: msg})
		fmt.Fprintf(w, "%s\n", b)
	} else {
		fmt.Fprintln(w, msg)
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
