#!/usr/bin/env bash
# apt-get install para os jobs do CI. Os .deb baixados ficam em APT_DEB_CACHE (cache do Actions):
# o download nos mirrors do Ubuntu chegou a levar 26 min; instalar os pacotes leva ~20 s.
# Sem o gatilho do man-db: reindexar as man pages a cada pacote não serve a ninguém no CI.
set -euo pipefail
cache="${APT_DEB_CACHE:-$HOME/.cache/apt-debs}"
mkdir -p "$cache"
sudo rm -f /var/lib/man-db/auto-update
if compgen -G "$cache/*.deb" >/dev/null; then sudo cp --update=none "$cache"/*.deb /var/cache/apt/archives/; fi
sudo apt-get update -qq
sudo apt-get install -y -qq --no-install-recommends "$@"
