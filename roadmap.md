# Roadmap — 10 oportunidades para a Mika

Minha aposta: a Mika precisa menos de novas ferramentas e mais de continuidade, timing e confiança.

As propostas abaixo são evoluções apoiadas no código existente, não bugs confirmados. Nenhuma foi implementada nesta análise.

> Os `Base:` apontam para o backend Rust atual (`apps/desktop/src-tauri/src`), conferidos em 2026-10-06 — o backend TypeScript que a primeira versão citava foi apagado na migração para Tauri. Símbolos em vez de números de linha: linha envelhece a cada commit.

## 1. “Onde paramos?” ao reabrir um jogo

Transformar o histórico em uma retomada curta: último acontecimento, objetivo pendente e local visitado. O resumo já existe; a evolução é usá-lo na volta.

**Base:** `apps/desktop/src-tauri/src/modules/gamer/session_memory.rs` (`SessionMemory::session_for_game`, `set_summary`).

## 2. Mapa das suas descobertas, não um walkthrough

Exibir locais, conexões e itens registrados numa jornada navegável, sem revelar áreas desconhecidas.

**Base:** `apps/desktop/src-tauri/src/modules/gamer/game_locations.rs` e `apps/desktop/src-tauri/src/modules/gamer/session_memory.rs` (`save_location`, `find_locations`).

## 3. Escada de ajuda sem spoilers

Oferecer “uma pista”, depois “uma dica”, e só então “a solução”, respeitando uma preferência persistente por jogo. Já há níveis de spoiler por chamada.

**Base:** `apps/desktop/src-tauri/src/modules/gamer/game_knowledge_tool.rs` (`spoiler_level` por chamada).

## 4. Personalidade de companhia por jogo

Silenciosa em jogos narrativos, provocadora em partidas casuais, prestativa em exploração — perfis escolhidos pelo usuário, não ajustes globais constantes.

**Base:** `apps/desktop/src-tauri/src/config/schemas/gamer.rs` (namespace `gamer-companion`).

## 5. Aprender quando ficar quieta

Permitir feedback “boa hora” / “agora não” para ajustar a frequência dos comentários por contexto, com opção de resetar. Evolui os cooldowns e a probabilidade configurada.

**Base:** `apps/desktop/src-tauri/src/config/schemas/vision.rs` (cooldowns/probabilidade) e `apps/desktop/src-tauri/src/modules/gamer/mute_learning.rs`.

## 6. “Isso mudou”: memória corrigível

Além de esquecer, permitir substituir uma preferência antiga e mostrar qual informação foi atualizada. O usuário controla a própria história, não apenas sua retenção.

**Base:** `apps/desktop/src-tauri/src/config/schemas/mod.rs` (namespace `memory`) e `apps/desktop/src-tauri/src/modules/memory/episodic/service.rs` (`update`).

## 7. Modo passageiro: companhia sem lembranças

Uma sessão temporária que não persiste conversa, memória nem diário visual, com indicador permanente e restauração das preferências ao sair. Não confundir com modo offline.

**Base:** `apps/desktop/src-tauri/src/config/schemas/misc.rs` (namespace `privacy`) e `apps/desktop/src-tauri/src/modules/features/privacy/mod.rs`.

## 8. Prévia “é isto que estou vendo”

Mostrar os monitores/regiões autorizados e o destino local ou remoto antes de ativar visão; permitir excluir áreas privadas visualmente. Evolui o indicador de captura e a blocklist.

**Base:** `apps/desktop/src-tauri/src/config/schemas/vision.rs` (indicador de captura, blocklist).

## 9. “Por que você lembrou disso?”

Dar às lembranças usadas numa resposta uma origem consultável: conversa, sessão ou observação — distinguindo registro de inferência. Começar pelos eventos de jogo, que já têm data e contexto.

**Base:** `apps/desktop/src-tauri/src/modules/gamer/session_memory.rs` e `apps/desktop/src-tauri/src/modules/gamer/session_types.rs`.

## 10. Álbum de momentos compartilhados

Transformar eventos selecionados pelo usuário em cartões locais com narrativa da Mika: primeira vitória, tentativa engraçada, descoberta favorita. Diferente do recap: uma coleção afetiva, editável e exportável.

**Base:** `apps/desktop/src-tauri/src/modules/gamer/session_memory.rs` (eventos da sessão).

## Prioridade sugerida

Começar por **1, 3 e 7**: continuidade perceptível, ajuda sem estragar a descoberta e privacidade fácil de entender.
