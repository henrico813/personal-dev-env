# Codex Multi-Config System Makefile
# Manage Codex with different configurations

# Configuration
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
CONFIG_DIR := $(MAKEFILE_DIR)ai-profiles
SHELL := /bin/zsh

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[1;33m
RED := \033[0;31m
NC := \033[0m

# Default target - show help
.DEFAULT_GOAL := help

# ============================================================================
# Help & Information
# ============================================================================

.PHONY: help
help: ## Display all available commands with descriptions
	@echo "$(BLUE)Personal Dev Env - AI Tool Launcher$(NC)"
	@echo "=================================="
	@echo ""
	@echo "$(GREEN)Claude Code Commands:$(NC)"
	@grep -E '^claude:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make claude                                     # Local models (uses default model)"
	@echo "  make claude MODEL=openai/glm4.5-air-reap        # Specify model"
	@echo "  make claude MODEL=openai/qwen3-30b-a3b-thinking # Another model"
	@echo "  make claude PROFILE=default                     # Use Anthropic API"
	@echo ""
	@echo "$(GREEN)Note:$(NC) Model names must match those configured in your LiteLLM proxy"

# ============================================================================
# Claude Code Launcher
# ============================================================================

.PHONY: claude
claude: ## Launch Claude Code (PROFILE=local|default, MODEL=<model-name>)
	@profile=$${PROFILE:-local}; \
	if [ ! -f "$(CONFIG_DIR)/claude/$$profile.env" ]; then \
		echo "$(RED)Error:$(NC) Profile '$$profile' not found at $(CONFIG_DIR)/claude/$$profile.env"; \
		echo "$(YELLOW)Available profiles:$(NC)"; \
		ls -1 $(CONFIG_DIR)/claude/*.env | xargs -n1 basename | sed 's/.env//' | sed 's/^/  /'; \
		exit 1; \
	fi; \
	launch_dir="$(LAUNCH_DIR)"; \
	if [ -z "$$launch_dir" ]; then \
		launch_dir=$$(pwd); \
	fi; \
	if [ -n "$(MODEL)" ]; then \
		echo "$(BLUE)Launching Claude Code with profile '$$profile' and model '$(MODEL)'$(NC)"; \
		source $(CONFIG_DIR)/claude/$$profile.env && cd "$$launch_dir" && claude --model $(MODEL); \
	else \
		echo "$(BLUE)Launching Claude Code with profile '$$profile'$(NC)"; \
		source $(CONFIG_DIR)/claude/$$profile.env && cd "$$launch_dir" && \
		if [ -n "$$DEFAULT_MODEL" ]; then \
			echo "$(YELLOW)Using default model: $$DEFAULT_MODEL$(NC)"; \
			claude --model $$DEFAULT_MODEL; \
		else \
			claude; \
		fi; \
	fi

# ============================================================================
# Utility Targets
# ============================================================================

.PHONY: clean
clean: ## Remove any generated files
	@echo "$(BLUE)Cleaning up...$(NC)"
	@rm -f *.log
	@echo "$(GREEN)✓$(NC) Clean complete"

# Declare all targets as phony
.PHONY: help codex claude clean
