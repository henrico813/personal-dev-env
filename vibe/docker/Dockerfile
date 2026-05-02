FROM node:20-bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash git \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g @mariozechner/pi-coding-agent@0.71.1

COPY extensions /opt/vibe/extensions
COPY hooks /opt/vibe/hooks
COPY docker/run-agent.sh /opt/vibe/run-agent.sh

RUN chmod +x /opt/vibe/run-agent.sh /opt/vibe/hooks/post-commit

ENTRYPOINT ["/opt/vibe/run-agent.sh"]
