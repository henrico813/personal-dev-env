# Use Ubuntu as the base image
FROM ubuntu:22.04 as system

    # Set environment variables to avoid interactive prompts during package installation
    ENV DEBIAN_FRONTEND=noninteractive
    ENV USER=light

    # Update package lists and install necessary packages
    RUN apt-get update && \
        apt-get install -y  curl \ 
                            tar \
                            xz-utils \ 
                            sudo \
                            screen \
                            locales \
                            locales-all

FROM system as fonts

    RUN apt-get install -y  fontconfig \
                            fonts-dejavu-core \
                            fonts-dejavu-extra \
                            fonts-liberation \
                            ttf-mscorefonts-installer \
                            fonts-powerline

    RUN curl -OL https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.tar.xz

# Build system configuration
FROM fonts as config

    # Create a new user
    RUN useradd -ms /bin/bash $USER

    # Add the user to the sudoers file
    RUN echo "$USER ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

    # Generate and set locale
    RUN locale-gen en_US.UTF-8
    ENV LANG=en_US.UTF-8
    ENV LANGUAGE=en_US:en
    ENV LC_ALL=en_US.UTF-8

    # Switch to the new user
    USER $USER

    WORKDIR /home/$USER

# Install the personal dev environment
FROM config as pdemain

    # Copy the install script into the container
    COPY install.sh install.sh
    COPY .config/zsh/.zsh_plugins.txt .zsh_plugins.txt
    COPY .config/zsh/.zshrc/ .zshrc
    COPY .config .config
    COPY .config/tmux/.tmux.conf .tmux.conf
   
    # Make the install script executable
    RUN sudo chmod +x ./install.sh

    # Copy the screenrc file into the container
    COPY .config/screen/.screenrc /home/$USER/.screenrc

    # Run the install script
    RUN ./install.sh

    # neovim plugins
    RUN sudo chown -Rh $USER .
    RUN sh -c 'curl -fLo /home/$USER/.config/nvim/autoload/plug.vim --create-dirs \
        https://raw.githubusercontent.com/junegunn/vim-plug/master/plug.vim'

    # Start the container with a screen session
    CMD ["screen", "-S", "my_session", "-c", "/home/light/.screenrc", "-m", "/home/light/.nix-profile/bin/zsh"]
