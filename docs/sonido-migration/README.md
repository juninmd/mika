# Troca dos primitivos locais pelo Sonido

Os cinco primitivos compartilhados abaixo eram implementações locais de padrões que o
[`@juninmd/sonido`](https://github.com/juninmd/sonido) já publica — vários deles nasceram no próprio
mika (`PageHeader`, `StatTile`, `PresenceOrb`…). Nesta troca eles viram adaptadores finos: a API que
o app usa (`hint`, `variant`, `tone: 'ok' | 'warn'`, `icon: LucideIcon`) continua igual, e o
comportamento/ARIA vem do pacote.

| Componente     | Antes (mika)                                        | Depois (`@juninmd/sonido@2.2.1`)                                 |
| -------------- | --------------------------------------------------- | --------------------------------------------------------------- |
| `StatusPill`   | `<span>` próprio, 10 variantes, ponto pulsando      | `Badge` com `tone`, `dot` e `live`; variantes traduzidas         |
| `StatTile`     | `<div>/<button role="status">` com 6 tons            | `StatTile` com `icon`, `tone` e valor animado; `role` no wrapper |
| `EmptyState`   | `<div role="status">` com ícone animado e frase     | `EmptyState` com título, descrição e ação                        |
| `ConfirmButton`| botão + `fieldset` de confirmação próprios          | `ConfirmButton` (mesmo fluxo, Esc e foco vêm do Sonido)          |
| `PageHero`     | `motion.section` com ícone, pílulas, aside e nota   | `PageHeader` (`titleAs="h2"`)                                    |

Arquivos alterados em `packages/sdk-ui/src/components/`: `StatusPill.tsx`, `StatTile.tsx`,
`EmptyState.tsx`, `ConfirmButton.tsx`, `PageHero.tsx`. Nenhum call site precisou mudar de props —
os 50+ usos de `StatusPill`, 23 de `ConfirmButton`, 15 de `StatTile`, 11 de `EmptyState` e 15 de
`PageHero` continuam importando de `@mika/sdk-ui/settings`.

## Tema

`data-theme` nunca teve consumidor no CSS do mika; agora é a chave de tema do Sonido, gravada em
`apply-theme.ts`: `light`/`sakura → light`, `aurora → aurora`, todo o resto → `dark`. O tema antigo
continua em `data-mika-theme`, e a paleta do app segue nos tokens inline (`--bg`, `--accent`, …).
O CSS do Sonido (o `styles.css` do pacote) entra uma vez em `renderer/src/main.tsx`, antes de
`styles.css`, para que a base do app (Tailwind + fontes empacotadas) tenha a última palavra.

## O que ficou de fora

- `GlassCard`, `GlassSection`, `BentoGrid`, `SectionHeader`, `StatusPill`-adjacent `SectionLabel`:
  são a identidade visual do app (vidro, `accentHue` por seção, grid do Bento). O `Card` do Sonido
  não tem `accentHue` nem o encaixe de `span`, então trocar seria redesenhar as telas, não migrar.
- `AuroraField`, `GlassSelect`, `GlassSlider`, `GlassToggle`: `AGENTS.md` exige que todo controle
  de formulário passe por `@mika/sdk-ui/settings` (gate `check:ui-controls`); o Sonido não substitui
  esse contrato de gravação/validação.
- `Toast`/`Spinner`: o app usa notificações próprias ligadas ao store e spinners do `lucide-react`;
  sem cópia local equivalente para comparar.

## Como as imagens foram geradas

`preview.html` + `preview.tsx` montam a mesma página com os dois lados. O lado **Antes** importa
cópias literais de `origin/main` em `legacy/` (só o caminho dos imports relativos mudou); o lado
**Depois** importa os arquivos do app já apoiados no Sonido. A página carrega o CSS real do app
(`apps/desktop/src/renderer/src/styles.css`, com Tailwind 4 e as fontes) mais o CSS do Sonido.

```bash
pnpm --filter @mika/desktop exec vite -c vite.sonido-preview.config.ts
# http://localhost:5302/preview.html

node C:/Users/jr_ac/AppData/Local/Temp/opencode/shot/full.cjs \
  "http://localhost:5302/preview.html" \
  "D:/Solutions/pessoal/wt-sonido/mika/docs/sonido-migration/antes-depois.png"

# variante com as duas perguntas do ConfirmButton abertas
node C:/Users/jr_ac/AppData/Local/Temp/opencode/shot/full.cjs \
  "http://localhost:5302/preview.html" \
  "D:/Solutions/pessoal/wt-sonido/mika/docs/sonido-migration/confirmacao.png" \
  confirm
```

As capturas usam Chromium 1180×900, `deviceScaleFactor: 2`, `fullPage`, com dados fictícios
(“Visão geral”, 128 mensagens, 42 memórias, provedor Ollama).
