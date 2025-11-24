# Export nvm completion settings for lukechilds/zsh-nvm plugin
# Note: This must be exported before the plugin is bundled
export NVM_DIR=${HOME}/.nvm
export NVM_COMPLETION=true

# Disable terminal flow control to enable Ctrl-S for forward history search
stty -ixon

# Plugin management using antidote
source ${ZDOTDIR:-$HOME}/.local/share/antidote/antidote.zsh
antidote load ${ZDOTDIR:-$HOME}/.zsh_plugins.txt

# Fix Ctrl-R and Ctrl-S for history search (must be after plugins load)
bindkey '^R' history-incremental-search-backward
bindkey '^S' history-incremental-search-forward

# Bundle zsh plugins via antibody
alias update-antibody='antibody bundle < $HOME/.zsh_plugins.txt > $HOME/.zsh_plugins.sh'
# List out all globally installed npm packages
alias list-npm-globals='npm list -g --depth=0'
# Adds better handling for `rm` using trash-cli
# https://github.com/sindresorhus/trash-cli
# You can empty the trash using the empty-trash command
# https://github.com/sindresorhus/empty-trash-cli
alias rm='trash'
# use neovim instead of vim
alias nvim='/opt/nvim'
alias vim='nvim'
# checkout branch using fzf
alias gcob='git branch | fzf | xargs git checkout'
# open vim config from anywhere
alias vimrc='vim ${HOME}/.config/nvim/init.vim'
# cat -> bat
alias cat='batcat'
# colored ls output
alias ls='eza -al'
# fdfind -> fd
alias fd='fdfind'
# yazi file manager
alias yazi='~/.cargo/bin/yazi'
# codex shortcut for neuro20b profile
function ask() {
    # Use --output-last-message to get only the clean formatted AI response
    codex e -p neuro20b "$@" --color always --output-last-message /tmp/ask_response.txt 2>/dev/null
    cat /tmp/ask_response.txt
}

# DIRCOLORS (MacOS)
export CLICOLOR=1

# FZF
export FZF_DEFAULT_COMMAND="rg --files --hidden --glob '!.git'"
export FZF_DEFAULT_OPTS="--height=40% --layout=reverse --border --margin=1 --padding=1"

# PDE Configuration - Load install paths
if [[ -f "${HOME}/.config/pde/paths.env" ]]; then
    source "${HOME}/.config/pde/paths.env"
fi

# PATH
# nvim path may help git use nvim
export PATH="$PATH:/opt/nvim/"
# use local, useful for aider
export PATH="$PATH:$HOME/.local/bin"
# add rust to path
export PATH="$PATH:$HOME/.cargo/bin"
export PATH="$PATH:$HOME/.rustup/bin"
export PATH="$HOME/.cargo/bin:$PATH"

# export PATH=${PATH}:/usr/local/go/bin
# export PATH=${PATH}:${HOME}/go/bin

export BAT_THEME="gruvbox-dark"

# EDITOR
export EDITOR=$(which nvim)

# LITELLM
export LITELLM_MASTER_KEY=sk-1234

# nix
#if [ -e ~/.nix-profile/etc/profile.d/nix.sh ]; then . ~/.nix-profile/etc/profile.d/nix.sh; fi

# To customize prompt, run `p10k configure` or edit ~/.p10k.zsh.
[[ ! -f ~/.p10k.zsh ]] || source ~/.p10k.zsh

# Increase the limit of commands held in the history and enable realtime sharing between
# multiple zsh sessions.
HISTFILE=~/.zsh_history
HISTSIZE=1000000
SAVEHIST=1000000

# History options
setopt SHARE_HISTORY          # Share history between sessions
setopt HIST_VERIFY            # Show command before executing from history expansion
setopt EXTENDED_HISTORY       # Save timestamp and duration with each command
setopt APPEND_HISTORY         # Append to history file rather than overwriting
setopt INC_APPEND_HISTORY     # Write to history file immediately, not on exit

# AI Tools CLI - Unified launcher for claude
ai() {
    if [ -z "$1" ]; then
        echo "Usage: ai <tool> [model|profile]"
        echo ""
        echo "Available tools:"
        echo "  claude [model]     - Launch Claude Code with local models"
        echo "  claude default     - Launch Claude Code with Anthropic API"
        echo ""
        echo "Examples:"
        echo "  ai claude"
        echo "  ai claude openai/glm4.5-air-reap"
        echo "  ai claude openai/qwen3-30b-a3b-thinking"
        echo "  ai claude default"
        return 1
    fi

    local tool=$1
    local arg=$2

    case $tool in
        aider)
            aider --subtree-only
            ;;

        claude)
            if [ -z "$PDE_INSTALL_PATH" ]; then
                echo "Error: PDE not configured. Config file missing at ~/.config/pde/paths.env"
                echo "Please run the Ansible playbook to configure your environment."
                return 1
            fi

            if [ ! -f "$PDE_INSTALL_PATH/Makefile" ]; then
                echo "Error: PDE Makefile not found at $PDE_INSTALL_PATH/Makefile"
                echo "The repository may have moved. Update ~/.config/pde/paths.env with the correct path."
                return 1
            fi

            # Capture current directory before make changes it
            local current_dir=$(pwd)

            # If no argument or argument is "default", use PROFILE
            # Otherwise treat argument as MODEL
            if [ -z "$arg" ]; then
                # No argument: use local profile with default model
                make -C ${PDE_INSTALL_PATH} claude LAUNCH_DIR="$current_dir"
            elif [ "$arg" = "default" ]; then
                # Use official Anthropic API
                make -C ${PDE_INSTALL_PATH} claude PROFILE=default LAUNCH_DIR="$current_dir"
            else
                # Treat argument as model name
                make -C ${PDE_INSTALL_PATH} claude MODEL="$arg" LAUNCH_DIR="$current_dir"
            fi
            ;;
        *)
            echo "Unknown tool: $tool"
            echo "Available tools: claude"
            return 1
            ;;
    esac
}
