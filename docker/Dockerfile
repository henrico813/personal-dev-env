FROM ubuntu:22.04 as system

    ARG DEBIAN_FRONTEND=noninteractive

    # Install necessary packages
    RUN apt update
    RUN apt install -y \
        xfce4 \
        xfce4-goodies \
        xfce4-terminal \
        tigervnc-standalone-server \
        novnc \ 
        websockify \
        xvfb \
        xauth \
        sudo \
        curl \
        nginx

FROM system as environment

    # Install developer environment packages
    RUN apt install -y \
        python3 \
        python3-pip \
        tmux \
        vim \
        make \
        git
    
    RUN pip install \
        pipenv \
        flake8

FROM environment as display

    # Create a non-root user
    RUN adduser --gecos "" user && \
        echo "user:password" | chpasswd
    RUN usermod -aG sudo user

    # Set up the VNC server
    USER user
    RUN mkdir -p /home/user/.vnc
    RUN echo "${VNC_PASSWORD:-password}" | vncpasswd -f > /home/user/.vnc/passwd
    RUN chmod 600 /home/user/.vnc/passwd

    # Create a startup script
    RUN echo '#!/bin/bash\n' \
            'export DISPLAY=:1\n' \
            'sleep 2\n' \
            'echo "Starting xfce4..."\n' \
            'startxfce4 &\n' \
            'sleep 2\n' \
            'echo "Starting VNC server..."\n' \
            'vncserver :1 -geometry ${VNC_GEOMETRY:-1280x800} -depth 24 -localhost no -passwd /home/user/.vnc/passwd > /home/user/vnc.log 2>&1\n' \
            'sleep 2\n' \
            'echo "Starting websockify..."\n' \
            'websockify --web=/usr/share/novnc/ ${VNC_PORT:-8080} localhost:5901\n' \
            'xfce4-panel --restart' \
            > /home/user/startup.sh \
        && chmod +x /home/user/startup.sh

    # Ensure .Xauthority is created
    RUN touch /home/user/.Xauthority

    EXPOSE ${VNC_PORT:-8080}

FROM display as usercfg

    LABEL maintainer="hco"
    LABEL version="0.3.0"
    LABEL description="This is a personal development environment in Docker"

    FROM usercfg as runner

    ARG DOCKER_IMAGE_VERSION

    CMD ["/bin/bash", "-c", "/home/user/startup.sh"]
    