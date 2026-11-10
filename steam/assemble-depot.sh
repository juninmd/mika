#!/usr/bin/env bash
# Monta a pasta de um depot da Steam a partir de `tauri build --no-bundle --features steam`.
# Uso: steam/assemble-depot.sh <windows|linux> <pasta-de-saída>   (na raiz do repositório)
#
# `bundle.resources` (prompts/ e legal/) chega a `target/release/` pelo `build.rs` (tauri-build), com ou
# sem `--no-bundle`; o que `--no-bundle` pula é o bundler, que é quem os poria no layout final do
# pacote. Então o depot os leva à mão, no lugar onde `resource_dir` (tauri-utils) os procura:
#  - Windows: ao lado do exe (`prompts/`, `legal/`);
#  - Linux: `<exe>/../lib/<productName>` — productName "Mika", NÃO o nome do crate (tauri-codegen
#    usa `product_name`) — daí `bin/` + `lib/Mika/` no depot Linux.
# `legal/` é lido de `BaseDirectory::Resource` por `modules/legal.rs` (EULA, privacidade, notices).
set -euo pipefail

os="${1:?uso: assemble-depot.sh <windows|linux> <saída>}"
out="${2:?uso: assemble-depot.sh <windows|linux> <saída>}"
release=target/release
prompts="$release/prompts"
legal="$release/legal"

need() { [ -e "$1" ] || { echo "::error::depot $os: '$1' não existe"; exit 1; }; }
first() { compgen -G "$1" | head -n1 || true; }

copy_native_libs() { # $1 = destino, $2 = padrão; DLLs/SOs que o build gerou (ONNX Runtime, DirectML...), sem o cdylib do app
  local f
  for f in $release/$2; do
    [ -e "$f" ] || continue
    case "$(basename "$f")" in *mika_desktop_lib*) continue ;; esac
    cp "$f" "$1/"
  done
}

copy_notices() { # licenças e avisos de terceiros, se o repositório já os tem
  local f
  for f in LICENSE* THIRD-PARTY-NOTICES* docs/THIRD-PARTY-NOTICES*; do
    if [ -f "$f" ]; then cp "$f" "$1/"; fi
  done
}

rm -rf "$out"
need "$prompts"
need "$legal"

case "$os" in
  windows)
    mkdir -p "$out/_CommonRedist/vcredist" "$out/_CommonRedist/webview2"
    need "$release/mika-desktop.exe"
    cp "$release/mika-desktop.exe" "$out/"
    steam_api="$(first "$release/build/steamworks-sys-*/out/steam_api64.dll")"
    need "${steam_api:-missing-steam_api64.dll}"
    cp "$steam_api" "$out/"
    copy_native_libs "$out" '*.dll'
    cp -r "$prompts" "$out/prompts"
    cp -r "$legal" "$out/legal"
    cp steam/installscript.vdf "$out/"
    # Redistribuíveis da Microsoft, instalados em silêncio por `installscript.vdf` (idempotentes).
    curl -fsSL -o "$out/_CommonRedist/vcredist/vc_redist.x64.exe" https://aka.ms/vs/17/release/vc_redist.x64.exe
    curl -fsSL -o "$out/_CommonRedist/webview2/MicrosoftEdgeWebview2Setup.exe" 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'
    copy_notices "$out"
    ;;
  linux)
    mkdir -p "$out/bin" "$out/lib/Mika"
    need "$release/mika-desktop"
    cp "$release/mika-desktop" "$out/bin/"
    steam_api="$(first "$release/build/steamworks-sys-*/out/libsteam_api.so")"
    need "${steam_api:-missing-libsteam_api.so}"
    cp "$steam_api" "$out/bin/"
    copy_native_libs "$out/bin" '*.so*'
    cp -r "$prompts" "$out/lib/Mika/prompts"
    cp -r "$legal" "$out/lib/Mika/legal"
    copy_notices "$out"
    ;;
  *) echo "SO desconhecido: $os" >&2; exit 2 ;;
esac

echo "depot $os:"
(cd "$out" && find . -type f | sort | head -n 60)
