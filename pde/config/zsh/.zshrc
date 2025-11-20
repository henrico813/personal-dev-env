# Export nvm completion settings for lukechilds/zsh-nvm plugin
# Note: This must be exported before the plugin is bundled
export NVM_DIR=${HOME}/.nvm
export NVM_COMPLETION=true

# Plugin management using antidote
source ${ZDOTDIR:-$HOME}/.local/share/antidote/antidote.zsh
antidote load ${ZDOTDIR:-$HOME}/.zsh_plugins.txt

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
# aider shortcut (moved to ai function below) 

# DIRCOLORS (MacOS)
export CLICOLOR=1

# FZF
export FZF_DEFAULT_COMMAND="rg --files --hidden --glob '!.git'"
export FZF_DEFAULT_OPTS="--height=40% --layout=reverse --border --margin=1 --padding=1"

# PATH
# nvim path may help git use nvim
export PATH="$PATH:/opt/nvim/"
# use local, useful for aider
export PATH="$PATH/.local/bin"
# add rust to path
export PATH="$PATH:/.cargo/bin"
export PATH="$PATH:/.rustup/bin"
export PATH="$HOME/.cargo/bin:$PATH"

# export PATH=${PATH}:/usr/local/go/bin
# export PATH=${PATH}:${HOME}/go/bin

export BAT_THEME="gruvbox-dark"

# EDITOR
export EDITOR=$(which nvim)

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
            if [ -z "$PERSONAL_DEV_ENV" ]; then
                echo "Error: PERSONAL_DEV_ENV not set"
                return 1
            fi

            # Capture current directory before make changes it
            local current_dir=$(pwd)

            # If no argument or argument is "default", use PROFILE
            # Otherwise treat argument as MODEL
            if [ -z "$arg" ]; then
                # No argument: use local profile with default model
                make -C ${PERSONAL_DEV_ENV} claude LAUNCH_DIR="$current_dir"
            elif [ "$arg" = "default" ]; then
                # Use official Anthropic API
                make -C ${PERSONAL_DEV_ENV} claude PROFILE=default LAUNCH_DIR="$current_dir"
            else
                # Treat argument as model name
                make -C ${PERSONAL_DEV_ENV} claude MODEL="$arg" LAUNCH_DIR="$current_dir"
            fi
            ;;
        *)
            echo "Unknown tool: $tool"
            echo "Available tools: claude"
            return 1
            ;;
    esac
}
