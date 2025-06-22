# Export nvm completion settings for lukechilds/zsh-nvm plugin
# Note: This must be exported before the plugin is bundled
export NVM_DIR=${HOME}/.nvm
export NVM_COMPLETION=true

source ${HOME}/.zsh_plugins.sh

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
alias vim='nvim'
# checkout branch using fzf
alias gcob='git branch | fzf | xargs git checkout'
# open vim config from anywhere
alias vimrc='vim ${HOME}/.config/nvim/init.vim'
# cat -> bat
alias cat='bat'
# colored ls output
alias ls='ls -al --color'

# DIRCOLORS (MacOS)
export CLICOLOR=1

# FZF
export FZF_DEFAULT_COMMAND="rg --files --hidden --glob '!.git'"
export FZF_DEFAULT_OPTS="--height=40% --layout=reverse --border --margin=1 --padding=1"

# PATH
# export PATH=${PATH}:/usr/local/go/bin
# export PATH=${PATH}:${HOME}/go/bin

export BAT_THEME="gruvbox-dark"

# nix
if [ -e ~/.nix-profile/etc/profile.d/nix.sh ]; then . ~/.nix-profile/etc/profile.d/nix.sh; fi

# To customize Spaceship Prompt installed by antibody:
# Configure right prompt order
SPACESHIP_RPROMPT_ORDER=(
  user          # Username section
  host          # Hostname section
  time          # Time section (optional)
)

# Enable user section
SPACESHIP_USER_SHOW=always
SPACESHIP_USER_PREFIX=""
SPACESHIP_USER_SUFFIX=""
SPACESHIP_USER_COLOR="yellow"

# Enable host section  
SPACESHIP_HOST_SHOW=always
SPACESHIP_HOST_PREFIX="@"
SPACESHIP_HOST_SUFFIX=""
SPACESHIP_HOST_COLOR="green"

# Optional: Add time
SPACESHIP_TIME_SHOW=true
SPACESHIP_TIME_PREFIX=" "
SPACESHIP_TIME_SUFFIX=""
SPACESHIP_TIME_COLOR="blue"

# Increase the limit of commands held in the history and enable realtime sharing between
# multiple zsh sessions.
HISTFILE=~/.zsh_history
HISTSIZE=1000000
SAVEHIST=1000000
setopt SHARE_HISTORY
