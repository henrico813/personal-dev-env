# .bash_profile

# Source the .profile if it exists
if [ -f "$HOME/.profile" ]; then
    source "$HOME/.profile"
fi

# Source the .bashrc if it exists
if [ -f "$HOME/.bashrc" ]; then
    source "$HOME/.bashrc"
fi
