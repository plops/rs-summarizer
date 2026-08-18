#!/usr/bin/env sh
set -eu

# Bind to host loopback (127.0.0.1) and docker0 bridge (172.17.0.1)
# so the service is only accessible locally and by local Docker containers,
# never exposed on external/public network interfaces.
DOCKER_BRIDGE_IP=$(ip -4 addr show dev docker0 2>/dev/null | awk '/inet / {print $2}' | cut -d/ -f1 || echo "172.17.0.1")
DOCKER_BRIDGE_IP=${DOCKER_BRIDGE_IP:-172.17.0.1}

sudo docker rm -f ytdlp-pot-provider 2>/dev/null || true

sudo docker run -d \
  --name ytdlp-pot-provider \
  --restart unless-stopped \
  -p 127.0.0.1:4416:4416 \
  -p "${DOCKER_BRIDGE_IP}:4416:4416" \
  brainicism/bgutil-ytdlp-pot-provider:latest
