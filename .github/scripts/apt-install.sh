#!/usr/bin/env bash
# apt-get install para os jobs do CI, sem o gatilho do man-db: reindexar as man pages a cada pacote
# instalado era a maior parte dos minutos do passo "Dependências do sistema".
set -euo pipefail
sudo rm -f /var/lib/man-db/auto-update
sudo apt-get update -qq
sudo apt-get install -y -qq --no-install-recommends "$@"
