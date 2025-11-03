# Claude Code Multi-Profile System Makefile
# Manage Claude Code with different profiles

# Configuration
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
PROFILE_DIR := $(MAKEFILE_DIR)profiles
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
	@echo "$(BLUE)Claude Code Profile System$(NC)"
	@echo "=================================="
	@echo ""
	@echo "$(GREEN)Main Command:$(NC)"
	@grep -E '^claude:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Profile Management:$(NC)"
	@grep -E '^(list-profiles|create-profile):.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-35s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make claude                          # Default Anthropic endpoint"
	@echo "  make claude PROFILE=litellm-local    # Custom profile with its model"
	@echo "  make create-profile PROFILE=litellm-local"
	@echo ""
	@echo "$(GREEN)Note:$(NC) Model selection is configured within each profile's DEFAULT_MODEL setting"

# ============================================================================
# Main Claude Code Launcher
# ============================================================================

.PHONY: claude
claude: ## Launch Claude Code (default or PROFILE=<name> for custom)
	@if [ -z "$(PROFILE)" ]; then \
		echo "$(BLUE)Launching Claude Code with default profile$(NC)"; \
		source $(PROFILE_DIR)/default.env && claude; \
	else \
		if [ ! -f "$(PROFILE_DIR)/$(PROFILE).env" ]; then \
			echo "$(RED)Error:$(NC) Profile '$(PROFILE)' not found"; \
			echo ""; \
			$(MAKE) -s list-profiles; \
			exit 1; \
		fi; \
		echo "$(BLUE)Launching Claude Code with profile: $(PROFILE)$(NC)"; \
		source $(PROFILE_DIR)/$(PROFILE).env && \
		if [ -n "$$DEFAULT_MODEL" ]; then \
			echo "$(GREEN)Using model:$(NC) $$DEFAULT_MODEL"; \
			claude --model $$DEFAULT_MODEL; \
		else \
			claude; \
		fi; \
	fi

# ============================================================================
# Profile Management
# ============================================================================

.PHONY: list-profiles
list-profiles: ## Show all available profiles with descriptions
	@echo "$(BLUE)Available Profiles:$(NC)"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@for profile in $(PROFILE_DIR)/*.env; do \
		if [ -f "$$profile" ]; then \
			name=$$(basename "$$profile" .env); \
			desc=$$(grep "^PROFILE_DESCRIPTION=" "$$profile" 2>/dev/null | cut -d'"' -f2); \
			model=$$(grep "^DEFAULT_MODEL=" "$$profile" 2>/dev/null | cut -d'"' -f2); \
			if [ -n "$$model" ]; then \
				printf "  $(YELLOW)%-20s$(NC) %s $(GREEN)[%s]$(NC)\n" "$$name" "$$desc" "$$model"; \
			else \
				printf "  $(YELLOW)%-20s$(NC) %s\n" "$$name" "$$desc"; \
			fi; \
		fi; \
	done
	@echo ""
	@echo "Usage: make claude PROFILE=<name>"

.PHONY: create-profile
create-profile: ## Create new LiteLLM profile from template (PROFILE=<name>)
	@if [ -z "$(PROFILE)" ]; then \
		echo "$(RED)Error:$(NC) PROFILE argument required"; \
		echo "$(YELLOW)Usage:$(NC) make create-profile PROFILE=<name>"; \
		exit 1; \
	fi
	@if [ -f "$(PROFILE_DIR)/$(PROFILE).env" ]; then \
		echo "$(YELLOW)Warning:$(NC) Profile '$(PROFILE)' already exists"; \
		echo "Edit it at: $(PROFILE_DIR)/$(PROFILE).env"; \
		exit 1; \
	fi
	@cp $(PROFILE_DIR)/litellm-template.env $(PROFILE_DIR)/$(PROFILE).env
	@sed -i 's/PROFILE_NAME="litellm-custom"/PROFILE_NAME="$(PROFILE)"/' $(PROFILE_DIR)/$(PROFILE).env
	@sed -i 's/PROFILE_DESCRIPTION="LiteLLM proxy configuration"/PROFILE_DESCRIPTION="Custom LiteLLM profile: $(PROFILE)"/' $(PROFILE_DIR)/$(PROFILE).env
	@echo "$(GREEN)✓$(NC) Created new profile: $(PROFILE)"
	@echo ""
	@echo "$(YELLOW)⚠ IMPORTANT:$(NC) Edit your profile and uncomment these required lines:"
	@echo "  - export ANTHROPIC_BASE_URL"
	@echo "  - export ANTHROPIC_AUTH_TOKEN"
	@echo "  - DEFAULT_MODEL"
	@echo ""
	@echo "Edit your profile at:"
	@echo "  $(PROFILE_DIR)/$(PROFILE).env"
	@echo ""
	@echo "Then use it with:"
	@echo "  make claude PROFILE=$(PROFILE)"

# ============================================================================
# Utility Targets
# ============================================================================

.PHONY: clean
clean: ## Remove any generated files
	@echo "$(BLUE)Cleaning up...$(NC)"
	@rm -f *.log
	@echo "$(GREEN)✓$(NC) Clean complete"

# Declare all targets as phony
.PHONY: help claude list-profiles create-profile clean
