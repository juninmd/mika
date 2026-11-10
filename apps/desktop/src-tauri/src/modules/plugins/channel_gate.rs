//! Reconfirmação (2026-09-22) de `mika:plugin-rpc`/`mika:plugin-test` — achado decisivo que a
//! rodada anterior (cabeçalho de `mod.rs`, 2026-09-21d) não tinha: a razão de fundo não é só "é
//! código, não dado" — é que **reivindicar qualquer um dos dois canais HOJE é regressão ativa**,
//! provada pela própria forma como `bridge.rs` roteia.
//!
//! ## O achado: o corte é por CANAL inteiro, nunca por método/pluginId
//!
//! `bridge.rs::mika_invoke`/`mika_send` decidem assim: `if state.native.owns(&channel) { native }
//! else { sidecar }`. Não existe meio-termo — `NativeRegistry::owns` (`native/mod.rs`) é uma
//! pergunta binária por CANAL, e o próprio cabeçalho de `native/mod.rs` é explícito: "só o que
//! ninguém reclamou desce para o sidecar". Reivindicar `mika:plugin-rpc` ou `mika:plugin-test`
//! corta o sidecar Node fora da rota INTEIRA, não só dos ramos que o módulo Rust sabe responder.
//!
//! Isso muda a leitura de uma instrução central desta tarefa: "se o registro for vazio, o canal
//! ainda tem de ser reivindicado e responder o mesmo erro nomeado que o TS responde" presume que
//! devolver `HandlerMissing`/degradar nomeado é uma forma aceitável de "fechar o canal com
//! honestidade". Estruturalmente não é — é trocar uma resposta que FUNCIONA hoje (via sidecar) por
//! uma que sempre falha, para tudo que a rodada Rust não cobre. "Canal reivindicado entregando menos
//! que o original" (a frase que o cabeçalho de `mod.rs` já usava) deixou de ser um risco teórico e
//! virou uma contagem concreta abaixo.
//!
//! ## `mika:plugin-rpc` — RESOLVIDO em 2026-09-22; a tabela abaixo virou o denominador do gate
//!
//! **Esta seção descrevia um bloqueio e hoje descreve a medição que o destravou.** A conclusão
//! original ("reivindicar quebra os 4 registrantes reais, 25 métodos") valia com o registro nativo
//! vazio; os quatro donos foram portados e 24 dos 25 passaram a despachar, então o canal caiu
//! (`native.rs`). A tabela fica porque virou a lista que
//! `native_tests.rs::every_method_the_typescript_declares_is_registered_in_rust` cobra contra o
//! fonte do TS toda rodada — recusa por contagem tem de remedir a contagem, e esta agora remede
//! sozinha.
//!
//! Recontagem por `.rpc.handle(` (a rodada anterior já tinha achado os mesmos 4 donos; a correção
//! aqui é a contagem por método, que estava arredondada):
//!
//! | Plugin | Arquivo | Métodos |
//! |---|---|---|
//! | `gamer-companion` | `features/gamer-companion/handlers/gamer-companion-rpc.ts` | `session-list`, `session-current`, `steam:top-games`, `steam:game-art`, `mini-game:payload`, `mini-game:requested`, `mini-game:result` (7) |
//! | `mood` | `features/mood/index.ts` | `mood_get`, `mood_update`, `affinity_get`, `affinity_add_interaction` (4) |
//! | `person-catalog` | `features/person-catalog/handlers/person-catalog-rpc.ts` | `person_catalog_image`, `_list`, `_search`, `_get`, `_upsert`, `_merge`, `_block`, `_unblock`, `_blocked_list`, `_delete` (10 — a rodada anterior contou 9, o correto é 10) |
//! | `brain` (núcleo, não feature tardia) | `main/services/brain/brain-social-rpc.ts` | `brain_get_social_facts`, `_add_social_fact`, `_delete_social_fact`, `_update_social_fact` (4) |
//!
//! O argumento que valia aqui — reivindicar com o registro vazio faria os 25 métodos vivos (sessão
//! de jogo, humor/afinidade, CRUD de pessoas, fatos sociais) pararem de responder, e isso é
//! regressão medida, não "fechar com honestidade" — continua correto **como critério**. Ele só
//! deixou de se aplicar quando o registro encheu. Guardado porque é o critério que vale para
//! `mika:plugin-test` agora, e para qualquer canal que venha depois.
//!
//! ## `mika:plugin-test` — o ramo genérico não é "talvez zero", é OITO plugins reais
//!
//! A rodada anterior especulou que o ramo genérico (`plugin.definition.testConnection`) "pode ser
//! zero". Medido agora (`grep testConnection` em cada `features/<id>/index.ts`): **8 dos 9 late
//! features declaram** — `character`, `mcp`, `user`, `privacy`, `scheduler`, `person-catalog`,
//! `mood`, `gamer-companion` (Steam, via `testSteamConnection`). Só `desktop` não declara. Nenhum
//! tem equivalente em Rust. Reivindicar o canal quebraria o botão "Validar" desses 8 plugins.
//!
//! Os outros dois ramos, reconfirmados com o estado atual da árvore:
//! - **`llm`**: `brain::turn::run_turn_ollama_raw` (o cliente citado por esta tarefa, usado por
//!   `brain/tools/content.rs` e `collaboration_session.rs`) resolve config via
//!   `resolve_ollama_config(store)` sem parâmetro de provider — só Ollama. `testLlmConnection`
//!   (TS) resolve QUALQUER provider configurado via `resolveLanguageModelForProvider`. Mesma leitura
//!   da rodada anterior, símbolo diferente (`turn.rs`, não só `send.rs`), conclusão igual.
//! - **`mika.embeddings`**: desde 2026-09-24, `generate_embedding` chama Ollama de verdade
//!   (`{ollamaBaseUrl}/api/embeddings`) quando `llm.embeddingProvider` resolve para `ollama` — o
//!   transporte deixou de ser só teórico para ESSE provedor. Google/OpenRouter/OpenAI-compatible
//!   continuam sem cliente Rust. A conclusão de reivindicar o canal não muda (o corte é por canal
//!   inteiro, não por ramo/provider).
//!
//! ## `mika.voice` (dentro de `plugin-test`) — dois pedaços são portáveis sem rede
//!
//! Não muda a conclusão de reivindicar o canal (o corte é por canal inteiro, não por ramo), mas
//! muda o que fica pronto no dia em que o fallback abaixo existir:
//! - **STT local/browser** (`testSttProvider`, ramo `locality === 'local' | 'browser'`) é trivial —
//!   só lê config e monta uma mensagem fixa, sem rede. Zero peça faltando.
//! - **TTS Supertonic** já tem geração real em Rust: `voice/tts/dispatch.rs::generate_with_provider`
//!   é o MESMO caminho que `inference/supertonic.runtime.ts::testConnection` (TS) chama — gerar
//!   `"Oi! Esta é a minha voz, gerada aqui na sua máquina."` e devolver o áudio. Portável hoje.
//! - STT em nuvem (Groq/OpenAI) e os outros 6 provedores de TTS continuam sem porte (mesma leitura
//!   da rodada anterior).
//!
//! ## O caminho que esta seção propunha, e por que ele NÃO foi tomado
//!
//! Aqui estava descrito um escape hatch: reivindicar o canal, responder de verdade os ramos
//! portados e reencaminhar o resto a `sidecar.invoke`, "o MESMO sidecar que já responde hoje, só
//! que atrás de uma borda Rust". Era tecnicamente viável e foi recusado de propósito — **ele move
//! a rota sem matar o processo**, e era o processo que o objetivo mandava apagar. Teria inflado a
//! catraca sem reduzir dependência nenhuma.
//!
//! O que se fez no lugar: portar o que faltava. Os dois canais caíram em 2026-09-22 —
//! `mika:plugin-rpc` por `plugins/native.rs`, `mika:plugin-test` por `settings/mod.rs` — e o
//! sidecar foi apagado no mesmo dia, com a catraca em 43/43.
//!
//! **O que sobra deste arquivo é o critério, não a conclusão.** Ele continua certo: canal
//! reivindicado com registro vazio troca uma resposta que funciona por uma que sempre falha. O que
//! mudou foi o registro encher. Quem for reivindicar o próximo canal aplica a mesma régua — com a
//! diferença de que hoje não há mais sidecar para onde regredir, o que torna a medição ANTES de
//! reivindicar mais importante, não menos.

#[cfg(test)]
mod tests {
    const BRIDGE_RS: &str = include_str!("../../bridge.rs");
    const STATE_RS: &str = include_str!("../../state.rs");
    const NATIVE_MOD_RS: &str = include_str!("../../native/mod.rs");
    const TURN_RS: &str = include_str!("../brain/turn.rs");
    const EMBEDDING_RS: &str = include_str!("../memory/episodic/embedding.rs");

    /// O achado central: a decisão de roteamento é por canal inteiro, sem meio-termo por método.
    /// Se `bridge.rs` ganhar um roteamento mais fino (por `pluginId`/método), esta leitura muda e o
    /// teste tem de ser reescrito, não só ignorado.
    #[test]
    fn bridge_routes_by_whole_channel_never_by_method_or_plugin_id() {
        assert!(
            BRIDGE_RS.contains("state.native.owns(channel)"),
            "bridge.rs mudou o gate de roteamento — releia antes de reconsiderar reivindicar plugin-rpc/plugin-test"
        );
        // Era "bridge.rs ainda cai no sidecar para canal sem dono". Não cai mais: não há para
        // onde. O que substitui é mais forte — canal sem dono tem de virar ERRO NOMEADO. Enquanto
        // o fallback existia, um canal escrito errado funcionava assim mesmo pelo Node; agora ele
        // some da tela, e o único aviso que sobra é esta mensagem.
        assert!(
            BRIDGE_RS.contains("Err(unowned_channel(channel))"),
            "bridge.rs deixou de nomear canal sem dono — o engano volta a ser silencioso, sem sidecar para disfarçar"
        );
        assert!(
            !BRIDGE_RS.contains("state.sidecar"),
            "bridge.rs voltou a falar com um sidecar — ele foi apagado em 2026-09-22 e não deve ressuscitar por fallback"
        );
        assert!(
            NATIVE_MOD_RS.contains("pub fn owns(&self, channel: &str) -> bool"),
            "NativeRegistry::owns mudou de assinatura — a pergunta binária por canal pode ter ganhado granularidade"
        );
    }

    /// Este teste provava que o "escape hatch" existia: reivindicar um canal e reencaminhar ao
    /// sidecar os ramos sem porte era tecnicamente alcançável de dentro de um `NativeModule`. A
    /// opção foi RECUSADA por decisão (move a rota sem matar o processo), e em 2026-09-22 deixou
    /// de ser alcançável de todo — o sidecar não existe.
    ///
    /// Virou o oposto, e é a asserção que protege a decisão: nenhum módulo pode reabrir um caminho
    /// para um processo Node. Vale para o `Sidecar` antigo e para qualquer ressurreição dele.
    #[test]
    fn no_native_module_can_reach_a_node_process_anymore() {
        assert!(
            !STATE_RS.contains("pub sidecar"),
            "ShellState ganhou um sidecar de volta — a migração existe para não ter esse processo"
        );
        assert!(
            !std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sidecar.rs").exists(),
            "src/sidecar.rs voltou a existir"
        );
    }

    /// `run_turn_ollama_raw` — o cliente citado nesta tarefa — continua sem parâmetro de provider:
    /// só resolve config Ollama. Se ganhar um argumento de provider, o branch `llm` pode deixar de
    /// estar bloqueado por isto.
    #[test]
    fn run_turn_ollama_raw_still_has_no_provider_parameter() {
        let start = TURN_RS.find("pub async fn run_turn_ollama_raw(").expect("função sumiu de turn.rs");
        let signature_end = TURN_RS[start..].find(") -> Result").expect("fim da assinatura");
        let signature = &TURN_RS[start..start + signature_end];
        assert!(
            !signature.to_lowercase().contains("provider"),
            "run_turn_ollama_raw ganhou parâmetro de provider — releia o branch llm de mika:plugin-test"
        );
        assert!(TURN_RS.contains("resolve_ollama_config(store)"), "run_turn_ollama_raw parou de fixar Ollama via resolve_ollama_config");
    }

    /// O módulo existe e, desde 2026-09-24, `generate_embedding` tem cliente real para os QUATRO
    /// provedores (não só Ollama) — esta prova acompanha a mudança: o símbolo continua existindo, e
    /// o arquivo documenta que os quatro estão portados, não mais "nenhum" nem só um.
    #[test]
    fn embedding_module_exists_and_all_four_providers_have_a_real_client() {
        assert!(EMBEDDING_RS.contains("pub fn generate_embedding("), "generate_embedding sumiu de embedding.rs");
        assert!(
            EMBEDDING_RS.contains("Os quatro provedores têm cliente real"),
            "embedding.rs deixou de documentar que os quatro provedores estão portados"
        );
    }

    // Os 4 arquivos TS que davam o denominador de `mika:plugin-rpc` (25 métodos: 10 person-catalog
    // + 7 gamer-companion + 4 mood + 4 brain) foram removidos (backend Node deletado na migração
    // para Tauri). O denominador congelado e a prova de que nenhum método falta em Rust vivem em
    // `native_tests::FROZEN_TS_METHODS`/`every_frozen_ts_method_is_registered_in_rust`.

    // Os 9 arquivos TS que davam a contagem "8 de 9 late features declaram testConnection real"
    // foram removidos (backend Node deletado na migração para Tauri). A medição alimentava a
    // justificativa de por que `mika:plugin-test` continuava sem caminho honesto para ser
    // reivindicado (ver o cabeçalho deste arquivo e de `mod.rs`) — investigação registrada em
    // prosa, sem twin TS para remedir.
}
