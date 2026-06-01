GOLANGCI_LINT_VERSION := v2.11.4
GO_BIN := $(if $(GOBIN),$(GOBIN),$(shell go env GOPATH)/bin)
GOLANGCI_LINT := $(GO_BIN)/golangci-lint

.PHONY: check fix tidy init

check:
	@test -z "$$(gofmt -l .)" || { echo "unformatted files:"; gofmt -l .; exit 1; }
	$(GOLANGCI_LINT) run ./...
	go test ./...

fix:
	gofmt -w .
	$(GOLANGCI_LINT) run --fix ./...

tidy:
	go mod tidy

init:
	GOBIN="$(GO_BIN)" go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@$(GOLANGCI_LINT_VERSION)
