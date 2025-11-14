#!/bin/sh
CONTAINER_IP=$(hostname -i | awk '{print $1}')
socat TCP-LISTEN:1455,fork,reuseaddr,bind="$CONTAINER_IP" TCP:127.0.0.1:1455 >/tmp/codex-login.log 2>&1 &
codex login
