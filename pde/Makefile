# Makefile - Minimal version for claude launcher
# Full PDE installation: ./pde minimal|full

PROFILE ?= local
MODEL ?= openai/glm4.5-air-reap
LAUNCH_DIR ?= $(shell pwd)

.PHONY: claude
claude:
	cd $(LAUNCH_DIR) && claude --profile $(PROFILE) --model $(MODEL)

.PHONY: help
help:
	@echo "PDE Makefile - Claude launcher support"
	@echo ""
	@echo "Installation:"
	@echo "  ./pde minimal    Install shell tools (any machine)"
	@echo "  ./pde full       Install everything + AI tools (GUI machines)"
	@echo ""
	@echo "Claude launcher (used by ai() function):"
	@echo "  make claude                 Launch with default model"
	@echo "  make claude MODEL=<model>   Launch with specific model"
	@echo "  make claude PROFILE=default Launch with Anthropic API"
