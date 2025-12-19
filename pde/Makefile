# Personal Dev Environment - Profile-Based Deployment
# Makes it easy to deploy different configurations across your homelab

INVENTORY := inventory.yml
PLAYBOOK := site.yml
USER := $(shell whoami)

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m

.DEFAULT_GOAL := help

# ============================================================================
# Help & Information
# ============================================================================

.PHONY: help
help: ## Display all available commands
	@echo "$(BLUE)Personal Dev Environment - Profile-Based Deployment$(NC)"
	@echo "======================================================"
	@echo ""
	@echo "$(GREEN)Local Development Deployment:$(NC)"
	@grep -E '^deploy-dev.*?:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-25s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Remote Server Deployment:$(NC)"
	@grep -E '^deploy-server.*?:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-25s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Minimal Deployment:$(NC)"
	@grep -E '^deploy-minimal.*?:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-25s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Tag-Based Deployment:$(NC)"
	@grep -E '^deploy-.*-only.*?:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-25s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Utility Commands:$(NC)"
	@grep -E '^(list|check|update).*?:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-25s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make deploy-dev                          # Full dev setup on localhost"
	@echo "  make deploy-server HOST=pve1             # Server setup on pve1"
	@echo "  make deploy-shell-only HOST=blathers     # Just shell QoL on blathers"
	@echo "  make list-hosts                          # Show all configured hosts"

# ============================================================================
# Development Profile Deployment
# ============================================================================

.PHONY: deploy-dev
deploy-dev: ## Deploy full development environment (localhost)
	@echo "$(BLUE)Deploying Development Profile to localhost$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit development \
		--extra-vars "user=$(USER)"

.PHONY: deploy-dev-remote
deploy-dev-remote: check-host ## Deploy development environment to remote host (HOST=hostname)
	@echo "$(BLUE)Deploying Development Profile to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--extra-vars "pde_profile=development install_fonts=true install_dev_tools=true install_ai_tools=true"

# ============================================================================
# Server Profile Deployment
# ============================================================================

.PHONY: deploy-server
deploy-server: check-host ## Deploy server profile (shell QoL only) to remote host (HOST=hostname)
	@echo "$(BLUE)Deploying Server Profile to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--extra-vars "pde_profile=server install_fonts=false install_dev_tools=false install_ai_tools=false"

.PHONY: deploy-servers-all
deploy-servers-all: ## Deploy server profile to all servers in inventory
	@echo "$(BLUE)Deploying Server Profile to all servers$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) --limit servers

# ============================================================================
# Minimal Profile Deployment
# ============================================================================

.PHONY: deploy-minimal
deploy-minimal: check-host ## Deploy minimal profile (bare essentials) to remote host (HOST=hostname)
	@echo "$(BLUE)Deploying Minimal Profile to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--extra-vars "pde_profile=minimal install_fonts=false install_dev_tools=false install_ai_tools=false"

# ============================================================================
# Tag-Based Deployment (Granular Control)
# ============================================================================

.PHONY: deploy-shell-only
deploy-shell-only: check-host ## Deploy only shell configuration (HOST=hostname)
	@echo "$(BLUE)Deploying Shell Configuration to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--tags "deps,shell,config" \
		--skip-tags "fonts,tools"

.PHONY: deploy-fonts-only
deploy-fonts-only: check-host ## Deploy only fonts (HOST=hostname)
	@echo "$(BLUE)Deploying Fonts to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--tags "fonts"

.PHONY: deploy-tools-only
deploy-tools-only: check-host ## Deploy only development tools (HOST=hostname)
	@echo "$(BLUE)Deploying Development Tools to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--tags "tools"

.PHONY: update-shell-configs
update-shell-configs: check-host ## Update only shell config files (zshrc, tmux.conf, etc.) (HOST=hostname)
	@echo "$(BLUE)Updating Shell Configs on $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--tags "shell,config"

# ============================================================================
# Utility Commands
# ============================================================================

.PHONY: list-hosts
list-hosts: ## List all hosts in inventory
	@echo "$(BLUE)Available Hosts:$(NC)"
	@ansible-inventory -i $(INVENTORY) --list | jq -r '._meta.hostvars | keys[]' | grep -v '^_' | sort

.PHONY: list-groups
list-groups: ## List all inventory groups
	@echo "$(BLUE)Available Groups:$(NC)"
	@ansible-inventory -i $(INVENTORY) --graph

.PHONY: check-connectivity
check-connectivity: ## Check SSH connectivity to all hosts
	@echo "$(BLUE)Testing connectivity to all hosts$(NC)"
	ansible all -i $(INVENTORY) -m ping

.PHONY: check-host
check-host:
	@if [ -z "$(HOST)" ]; then \
		echo "$(YELLOW)Error: HOST variable is required$(NC)"; \
		echo "Usage: make deploy-server HOST=pve1"; \
		exit 1; \
	fi

.PHONY: dry-run
dry-run: check-host ## Dry run deployment (show what would change) (HOST=hostname)
	@echo "$(BLUE)Dry run deployment to $(HOST)$(NC)"
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) \
		--limit $(HOST) \
		--check \
		--diff

# ============================================================================
# Quick Deployment Shortcuts
# ============================================================================

.PHONY: pve1 pve2 blathers devlab
pve1: HOST=pve1
pve1: deploy-server ## Quick deploy server profile to pve1

pve2: HOST=pve2
pve2: deploy-server ## Quick deploy server profile to pve2

blathers: HOST=blathers
blathers: deploy-server ## Quick deploy server profile to blathers

devlab: HOST=devlab
devlab: deploy-dev-remote ## Quick deploy dev profile to devlab

.PHONY: clean
clean: ## Remove any generated files
	@echo "$(BLUE)Cleaning up...$(NC)"
	@rm -f *.retry
	@echo "$(GREEN)✓$(NC) Clean complete"
