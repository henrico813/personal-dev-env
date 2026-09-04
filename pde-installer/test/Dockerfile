ARG GO_VERSION=1.21
ARG BASE_IMAGE=ubuntu:24.04

FROM golang:${GO_VERSION} AS builder
WORKDIR /src/pde-installer
COPY pde-installer/go.mod pde-installer/go.sum ./
RUN go mod download
COPY pde-installer/ ./
RUN go build -o /out/pde-installer .

FROM ${BASE_IMAGE}
COPY --from=builder --chown=testuser:testuser /out/pde-installer /home/testuser/.local/bin/pde-installer
COPY --from=builder /usr/local/go /usr/local/go
ENV PATH="/home/testuser/.local/bin:/usr/local/go/bin:${PATH}"
COPY --chown=testuser:testuser . /home/testuser/personal-dev-env
USER testuser
WORKDIR /home/testuser/personal-dev-env
