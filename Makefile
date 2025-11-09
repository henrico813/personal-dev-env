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
	@echo "$(GREEN)Codex Commands:$(NC)"
	@grep -E '^codex:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Claude Code Commands:$(NC)"
	@grep -E '^claude:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Configuration Management:$(NC)"
	@grep -E '^(list-configs|create-config):.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make codex                                      # Default OpenAI configuration"
	@echo "  make codex CONFIG=my-litellm                    # Custom configuration"
	@echo "  make claude                                     # Local models (uses default model)"
	@echo "  make claude MODEL=openai/glm4.5-air-reap        # Specify model"
	@echo "  make claude MODEL=openai/qwen3-30b-a3b-thinking # Another model"
	@echo "  make claude PROFILE=default                     # Use Anthropic API"
	@echo "  make create-config CONFIG=my-litellm            # Create new codex config"
	@echo ""
	@echo "$(GREEN)Note:$(NC) Model names must match those configured in your LiteLLM proxy"

# ============================================================================
# Main Codex Launcher
# ============================================================================

.PHONY: codex
codex: ## Launch Codex (default or CONFIG=<name> for custom)
	@if [ -z "$(CONFIG)" ]; then \
		echo "$(BLUE)Launching Codex with default configuration$(NC)"; \
		cd $(CONFIG_DIR) && ln -sf default.toml config.toml; \
		CODEX_HOME=$(CONFIG_DIR) codex; \
	else \
		if [ ! -f "$(CONFIG_DIR)/$(CONFIG).toml" ]; then \
			echo "$(RED)Error:$(NC) Configuration '$(CONFIG)' not found"; \
			echo ""; \
			$(MAKE) -s list-configs; \
			exit 1; \
		fi; \
		echo "$(BLUE)Launching Codex with configuration: $(CONFIG)$(NC)"; \
		cd $(CONFIG_DIR) && ln -sf $(CONFIG).toml config.toml; \
		CODEX_HOME=$(CONFIG_DIR) codex; \
	fi

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
	if [ -n "$(MODEL)" ]; then \
		echo "$(BLUE)Launching Claude Code with profile '$$profile' and model '$(MODEL)'$(NC)"; \
		source $(CONFIG_DIR)/claude/$$profile.env && claude --model $(MODEL); \
	else \
		echo "$(BLUE)Launching Claude Code with profile '$$profile'$(NC)"; \
		source $(CONFIG_DIR)/claude/$$profile.env && \
		if [ -n "$$DEFAULT_MODEL" ]; then \
			echo "$(YELLOW)Using default model: $$DEFAULT_MODEL$(NC)"; \
			claude --model $$DEFAULT_MODEL; \
		else \
			claude; \
		fi; \
	fi

# ============================================================================
# Configuration Management
# ============================================================================

.PHONY: list-configs
list-configs: ## Show all available configurations
	@echo "$(BLUE)Available Configurations:$(NC)"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@for config in $(CONFIG_DIR)/*.toml; do \
		if [ -f "$$config" ]; then \
			name=$$(basename "$$config" .toml); \
			model=$$(grep -E "^model = " "$$config" 2>/dev/null | cut -d'"' -f2); \
			provider=$$(grep -E "^model_provider = " "$$config" 2>/dev/null | cut -d'"' -f2); \
			if [ -n "$$model" ] && [ -n "$$provider" ]; then \
				printf "  $(YELLOW)%-20s$(NC) Model: $(GREEN)%s$(NC), Provider: $(GREEN)%s$(NC)\n" "$$name" "$$model" "$$provider"; \
			elif [ -n "$$model" ]; then \
				printf "  $(YELLOW)%-20s$(NC) Model: $(GREEN)%s$(NC)\n" "$$name" "$$model"; \
			else \
				printf "  $(YELLOW)%-20s$(NC)\n" "$$name"; \
			fi; \
		fi; \
	done
	@echo ""
	@echo "Usage: make codex CONFIG=<name>"

.PHONY: create-config
create-config: ## Create new configuration from template (CONFIG=<name>)
	@if [ -z "$(CONFIG)" ]; then \
		echo "$(RED)Error:$(NC) CONFIG argument required"; \
		echo "$(YELLOW)Usage:$(NC) make create-config CONFIG=<name>"; \
		exit 1; \
	fi
	@if [ -f "$(CONFIG_DIR)/$(CONFIG).toml" ]; then \
		echo "$(YELLOW)Warning:$(NC) Configuration '$(CONFIG)' already exists"; \
		echo "Edit it at: $(CONFIG_DIR)/$(CONFIG).toml"; \
		exit 1; \
	fi
	@cp $(CONFIG_DIR)/litellm-template.toml $(CONFIG_DIR)/$(CONFIG).toml
	@echo "$(GREEN)✓$(NC) Created new configuration: $(CONFIG)"
	@echo ""
	@echo "$(YELLOW)⚠ IMPORTANT:$(NC) Edit your configuration and uncomment/set these values:"
	@echo "  - model = \"your-model-name\""
	@echo "  - model_provider = \"your-provider\""
	@echo "  - [model_providers.your-provider] section"
	@echo ""
	@echo "Edit your configuration at:"
	@echo "  $(CONFIG_DIR)/$(CONFIG).toml"
	@echo ""
	@echo "Then use it with:"
	@echo "  make codex CONFIG=$(CONFIG)"

# ============================================================================
# Utility Targets
# ============================================================================

.PHONY: clean
clean: ## Remove any generated files
	@echo "$(BLUE)Cleaning up...$(NC)"
	@rm -f *.log
	@echo "$(GREEN)✓$(NC) Clean complete"

# Declare all targets as phony
.PHONY: help codex claude list-configs create-config clean
