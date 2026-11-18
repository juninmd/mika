#!/usr/bin/env bash
# Copia os .deb que o apt baixou neste job para APT_DEB_CACHE, que o actions/cache/save grava.
set -euo pipefail
cache="${APT_DEB_CACHE:-$HOME/.cache/apt-debs}"
mkdir -p "$cache"
if compgen -G "/var/cache/apt/archives/*.deb" >/dev/null; then cp --update=none /var/cache/apt/archives/*.deb "$cache"/; fi
echo "apt-debs: $(find "$cache" -name '*.deb' | wc -l) pacote(s), $(du -sh "$cache" | cut -f1)"
