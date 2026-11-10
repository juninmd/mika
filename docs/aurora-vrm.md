# VRM oficial: Mika Aurora — Arcade

A Arcade aprovada em 2026-10-12 ocupa o caminho padrão
`apps/desktop/src/renderer/public/vrm/mika/mika.vrm`. O catálogo a apresenta como
**Mika Aurora (VRM)**. Configurações que usam `vrm/mika/mika.vrm` continuam apontando para o
modelo oficial; escolhas de outros modelos permanecem respeitadas.

## Origem e integridade

- Protótipo: `../mika-prototypes/mika-aurora-01`, commit `f90853a`.
- Fonte aprovada: `blender-renewal/models/arcade.vrm`.
- Fonte editável: `blender-renewal/blend/arcade.blend`, preservada no protótipo.
- SHA-256 oficial: `01b6c5c653f4302d6433ab5de9dab53c154decec8b29ecd233a9422a90b05582`.
- Tamanho: 15.476.592 bytes; 138.245 triângulos; VRM 1.0.

O arquivo foi promovido sem regeneração. A base anterior ficou preservada em
`scripts/generators/assets/mika-base.vrm`, com SHA-256
`12c2b97e95e700783a6a550dc0eee2d7880aeedccef9ae67bc4c5a2f0f2631a2`.
O gerador da Haru lê essa base, para que futuras regenerações não dependam do design oficial.
Créditos e termos da base estão em `LICENSE-ASSETS.md` e `THIRD-PARTY-NOTICES.md`.

## Integração no runtime

A roupa oficial é Arcade Varsity. A pose `peace` e a nova pose `22` usam as rotações de braço
aprovadas; `22` faz paz com as duas mãos. O runtime fecha os três segmentos do dedão ao aplicar
o gesto. A ação `22` está no contrato do SDK e no inventário Rust, com fallback no Live2D.

As cinco miniaturas existentes do VRM foram regeneradas com o adapter da Mika. O bundle Tauri
inclui `renderer/vrm/` como recurso no disco, além do frontend, para a inspeção de modelo em Rust.

## Reprodução da evidência visual

Na raiz do repositório:

```powershell
pnpm -s exec tsx scripts/generators/render-haru-preview.mts --model=vrm/mika/mika.vrm --destination=apps/desktop/e2e/screenshots/aurora-official --browser=msedge '--poses=peace,22,wave,bow,presenting,cheer' --expressions
pnpm -s exec tsx scripts/generators/render-vrm-previews.mts msedge 'original,hiyori,mao,mark,rice'
```

O primeiro comando captura o renderer real e grava `validation.json` com hash do modelo,
ações disponíveis, pixels visíveis e erros. Os PNGs cobrem postura neutra, seis poses,
vistas e expressões. As capturas geradas ficam fora do Git.

Essa prova usa o adapter WebGL da Mika; não equivale a uma sessão da janela nativa Tauri.
O E2E nativo deve ser executado quando solicitado, conforme `AGENTS.md`.

## Validação da integração em 2026-10-12

`pnpm lint`, `pnpm typecheck`, os 88 testes de unidade focados e `pnpm build` passaram.
O VRM copiado para `apps/desktop/out/renderer/vrm/mika/mika.vrm` possui o hash aprovado.
As 18 capturas do renderer passaram por revisão visual independente, com `errors: []`.

`pnpm gates` passou nos gates de invariantes, mas a etapa Rust terminou com 3.174 testes
aprovados, dois reprovados e cinco ignorados. As duas falhas são preexistentes em
`update_backup`: `rotation_keeps_only_the_two_most_recent_backups` e
`a_crashed_tmp_folder_is_swept_and_never_counts_as_a_backup`. Ambas tentam abrir diretórios
com `std::fs::File::open` para ajustar timestamps e recebem `PermissionDenied` no Windows.
Esse módulo não foi alterado. A validação geral permanece pendente dessas correções.
