# Directory Shortcuts
export PROJ=/home/hco/Projects
export VNV=$PROJ/VnV
export NW=$VNV/Nighthawk
export VERS=$VNV/Versiv
export VSTL=$VNV/VSTL

# Nighthawk Vars
export NW_SW_TEST=$NW/01-codebase/nighthawk-sw-test
export NW_SQUISH_DIR="$NW/03-tools/squish-for-qt-7.1.0"

# Python Vars
export PY_BASE_DIR="/usr/bin/python3.10"
export PY_LIB_DIR="/usr/lib/python3.10"

# QoL Vars
export LOG_DIR="/home/hco/Projects/VnV/Utility/logs"

# Exec Shortcuts
alias obsidian='/home/hco/Projects/.obsidian/Obsidian-1.6.5.AppImage'

# Poetry
export PATH="/home/hco/.local/bin:$PATH"

PROMPT_COMMAND='PS1_CMD1=$(git branch --show-current 2>/dev/null)'; PS1='\[\e[2m\]\t\[\e[0;91m\]|\[\e[38;5;208m\]\u@\H\[\e[91m\]:\[\e[0;41m\]\W\[\e[0;91m\]<\[\e[0;2m\]${PS1_CMD1}\[\e[0;91m\]>\[\e[97m\]\[\e[0m\]'
