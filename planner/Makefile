GOLANGCI_LINT_VERSION := v2.11.4

.PHONY: check fix tidy init

check:
	@test -z "$$(gofmt -l .)" || { echo "unformatted files:"; gofmt -l .; exit 1; }
	golangci-lint run ./...
	go test ./...

fix:
	gofmt -w .
	golangci-lint run --fix ./...

tidy:
	go mod tidy

init:
	go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@$(GOLANGCI_LINT_VERSION)
