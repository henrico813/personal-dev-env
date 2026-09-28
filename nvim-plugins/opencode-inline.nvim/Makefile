.PHONY: build test check

build:
	go build -o bin/opencode-inline-shim ./cmd/opencode-inline-shim

test:
	go test ./...

check:
	test -z "$$(gofmt -l .)"
	go vet ./...
	go test ./...
