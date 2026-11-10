//! Catálogo de plugins/features em Rust — `mika:plugin-rpc`, `mika:plugin-test`,
//! `mika:plugin-ui-catalog` e `mika:tools-list-by-plugin`.
//!
//! Vazio significa NÃO PORTADO.
//!
//! **O que trava estes quatro canais não é falta de motor JS.** Isso foi verificado e fechado:
//! `FEATURE_LOADERS` (`apps/desktop/src/main/infrastructure/features.ts`) é um `Record` estático de
//! `import()` com caminho literal, sem `readdirSync`/`glob`/`require()` em lugar nenhum, e
//! `userData/plugins/<id>` é só armazenamento de dados. Os 12 plugins são TODOS do bundle, nenhum
//! de terceiro. A única extensibilidade externa de verdade é MCP, que roda como processo do SO
//! separado e tem porte próprio (`modules/mcp`). Nenhuma engine JS (`rquickjs`/`boa`/`deno_core`)
//! entra neste binário.
//!
//! O que trava de verdade: os registros (`ToolRegistry`, `FeatureRegistryService`) só populam
//! depois que os módulos EXECUTAM. Cada módulo portado que registra suas tools nativamente enche
//! este catálogo — é o mesmo mecanismo que levou o placar de atrito de 43 para 9.
//!
//! ## Investigação por canal (2026-09-21c) — nenhum dos quatro é reivindicável hoje
//!
//! Cada handler foi lido por inteiro, não só pelo nome do canal — três canais desta migração já
//! foram reivindicados entregando menos que o original, e "nome parece um catálogo" não prova nada.
//!
//! - **`mika:plugin-rpc`** (`ipc/plugin-rpc.ts` → `plugin-rpc-registry.ts::dispatchPluginRpc`) não é
//!   um catálogo: despacha para `PluginRpcHandler = (args: unknown) => Promise<unknown> | unknown`,
//!   um closure arbitrário que CADA plugin registra dentro do próprio `activate(context)`
//!   (`context.rpc.handle(method, handler)`, ver `plugin-activation.ts`). Os 9 plugins tardios
//!   (`character`, `desktop`, `gamer-companion`, `mcp`, `mood`, `person-catalog`, `privacy`,
//!   `scheduler`, `user`) são 100% TypeScript ainda. Não há dado para portar — é código, e
//!   reivindicar o canal sem executar esse código é ou inventar um resultado (mock em produção,
//!   proibido pelo AGENTS.md) ou devolver falha para todo método que hoje funciona.
//!
//!   Fronteira já validada, mesmo sem reivindicar o canal: `dispatchPluginRpc` barra por
//!   `windowType` (`PLUGIN_RPC_ALLOWED_WINDOW_TYPES` = `main`/`settings`/`plugin-view`) e, quando a
//!   janela é `plugin-view`, exige `context.pluginId === request.pluginId` — uma janela de plugin só
//!   fala com o RPC do PRÓPRIO plugin. `assertOwnerSurface` (`ipc/surface-policy.ts`) NÃO se aplica
//!   aqui: ela libera só `main`/`settings`/`wizard` e BARRARIA `plugin-view`, que é justamente quem
//!   tem de usar este canal — aplicá-la quebraria a feature em vez de protegê-la. O guard que já
//!   existe é mais estrito que `assertOwnerSurface` para este caso (identidade do plugin chamador,
//!   não só tipo de janela), e `pluginId`/`method` passam por `pluginRpcRequestSchema.parse` antes
//!   de tocar o registro — a fronteira já está validada do lado TS.
//!
//! - **`mika:plugin-test`** (`ipc/plugin-connection-test.ts::executePluginTest`) roteia por
//!   `pluginId`/`tabId` para três caminhos reais: `generateText()` do AI SDK contra o provider LLM
//!   configurado, `testVoiceConnection` (STT/TTS reais) ou, no caso genérico, outro closure
//!   arbitrário — `plugin.definition.testConnection(...)`. `llm` já tem porte em Rust (fase 4 do
//!   plano), mas o canal é UM só para todos os `pluginId`: reivindicar e responder certo só para
//!   `pluginId === 'llm'` e falhar silenciosamente para voz/embeddings/plugin genérico é exatamente
//!   o defeito que esta tarefa nomeia — canal reivindicado entregando menos que o original.
//!
//! - **`mika:plugin-ui-catalog`** — **REIVINDICADO em 2026-09-21**, por
//!   `modules/settings/plugin_ui.rs`. A leitura abaixo, desta investigação, estava certa no
//!   mecanismo e errada na conclusão, e vale registrar a diferença porque ela se repete:
//!
//!   O canal devolve `featureRegistry.getUiCatalog()` — `{ views, settingsPanels }` da MESMA
//!   `FeatureRegistryService` que serve `getAllSettings()` por trás de `mika:settings-declarations`.
//!   Daí a inferência de que os dois canais tinham o mesmo bloqueio. Mas `getUiCatalog` lê
//!   `views`/`settingsPanels` e **nunca** `settings`: são campos diferentes do mesmo registro, e
//!   estar completo num não exige estar completo no outro. Uma varredura exaustiva do TS mostrou que
//!   só `gamer-companion` e `person-catalog` declaram qualquer view ou painel — ambos portados —
//!   enquanto `settings` depende das 15 abas. Registro compartilhado não é bloqueio compartilhado;
//!   a pergunta é qual CAMPO o canal lê.
//!
//! - **`mika:tools-list-by-plugin`** (`ipc/tools.ts` → `tool-access.ts::mapToolsByPlugin`) filtra
//!   `toolRegistry.getAll()` por `pluginId` — MESMO registro (não uma cópia) que serve
//!   `mika:tools-list`. `modules/tools.rs` (outra frente, fora deste escopo) já investigou esse
//!   registro e decidiu não reivindicar `tools-list`: o `ToolRegistry` nativo só tem as tools dos
//!   módulos Rust já portados, e `registerFeatureWithTools('mika.brain', BrainFeature)` (mais
//!   embeddings/voice/vision/browser e as tools de plugin via `extensions.registerTool`) ainda só
//!   roda no Node. Ler esse mesmo registro nativo incompleto por aqui, filtrado por `pluginId`,
//!   devolveria menos tools do que o plugin de verdade tem — o mesmo falso vazio, uma camada adiante.
//!   Reivindicar `-by-plugin` sem `tools-list` estar completo é o mesmo defeito com outro nome.
//!
//! Nenhum dos quatro tem hoje um caminho honesto em Rust. `modules()` continua vazio de propósito —
//! os testes abaixo prendem esta leitura contra o código-fonte citado, não contra memória: se
//! qualquer um dos arquivos mudar de forma, o teste correspondente quebra e força reler a decisão.
//!
//! ## Reconfirmação (2026-09-21d) — só `plugin-rpc` e `plugin-test`, mesma conclusão, motivo mais fino
//!
//! Escopo desta rodada: só estes dois canais (`plugin-ui-catalog` e `tools-list-by-plugin` ficam
//! com a leitura de cima, intocada). Duas premissas da investigação anterior tinham mudado — TTS
//! sendo portado e tools de brain/embeddings/browser em andamento em paralelo — então a leitura foi
//! refeita do zero contra o estado ATUAL da árvore, não só a favor da conclusão anterior.
//!
//! - **`mika:plugin-test` continua sem caminho honesto, e o motivo ficou mais claro por branch:**
//!   - **`llm`** — `testLlmConnection` (`plugin-connection-test.ts`) resolve QUALQUER provider
//!     configurado via `llmFactory.resolveLanguageModelForProvider(providerOverride, modelOverride)`.
//!     O único caminho de LLM em Rust é `brain::turn::run_turn_ollama`, chamado por
//!     `chat/send.rs::handle_chat_send` — nem o NOME da função nem o corpo de `send.rs` mencionam
//!     "provider" (checado: zero ocorrências). Reivindicar este branch respondendo só quando o
//!     provider configurado for Ollama, e devolvendo sucesso incorreto ou erro pros demais, é
//!     exatamente o defeito que a rodada anterior nomeou.
//!   - **`mika.voice`** — `testVoiceConnection` roteia por `testType`/provider configurado: teste de
//!     STT (`testSttProvider` — Whisper local, navegador, ou HTTP contra Groq/OpenAI) OU um entre 7
//!     provedores de TTS (`elevenlabs`/`azure`/`google`/`openai`/`piper`/`kokoro`/`supertonic`, mais
//!     o pseudo-provedor `system`). **Atualização no meio desta mesma investigação: TTS aterrissou em
//!     Rust** (`voice/tts/`, `pub mod tts;` em `voice/mod.rs` — o teste-âncora anterior pegou essa
//!     mudança e falhou, como projetado). Mas `voice/tts/dispatch.rs::CUT_PROVIDERS` só cobre
//!     Supertonic de verdade — `elevenlabs`/`openai-tts`/`azure-tts`/`google-tts`/`piper`/`kokoro`
//!     E `system` devolvem erro nomeado ("ainda não foi portado"), documentado no próprio cabeçalho
//!     do arquivo como corte deliberado, não mock. Isso já é uma divergência de comportamento para
//!     `system` (o TS devolve `ok:true` sem gerar áudio; o Rust devolveria erro). E o branch de STT
//!     (`testType === 'stt'`) não tem NENHUM equivalente em Rust — nem o caminho "trivial" (checar
//!     `locality === 'local'`/`'browser'` e devolver `ok:true` sem chamada nenhuma) existe hoje.
//!     Reivindicar o canal responderia certo só para TTS-Supertonic e errado/faltando para STT e
//!     para 6 dos 7 provedores de TTS — o mesmo defeito, delimitado mais fino agora que existe algo
//!     real para comparar.
//!   - **`mika.embeddings`** — zero módulo/símbolo de embedding em `src-tauri/src/modules` (grep
//!     amplo, sem resultado). Nada mudou aqui.
//!   - **Genérico (`plugin.definition.testConnection`)** — grep por `test_connection`/`testConnection`
//!     em todo `src-tauri/src/modules` não bate em nenhum arquivo. O candidato mais próximo seria
//!     `features/mood/mod.rs`, que só expõe `pub(crate) use tool::register_tools;` — nenhuma
//!     superfície de teste de conexão, nem RPC. Os outros dono do fechamento
//!     (`character`/`gamer-companion`/`mcp`/`person-catalog`/`privacy`/`scheduler`/`user`/`brain`/
//!     `vision`) seguem 100% TypeScript para este propósito específico — `gamer/mod.rs` chega mais
//!     perto (Steam por disco portado) mas o próprio módulo documenta que Steam Web API e sessão
//!     ficaram de fora por falta de `AppHandle` dentro de `ToolHandler::call`, o mesmo bloqueio.
//!
//! - **`mika:plugin-rpc` — os usuários REAIS do canal são mais estreitos do que a lista de "9
//!   plugins tardios" sugeria.** Recontagem por `ctx.rpc.handle(` em `features/`+`main/services/`:
//!   só 4 registrantes existem de verdade — `gamer-companion` (`session-list`, `session-current`,
//!   `steam:top-games`, `steam:game-art`, `mini-game:*`), `mood` (`mood_get`/`mood_update`/
//!   `affinity_get`/`affinity_add_interaction`), `person-catalog` (`person_catalog_*`, 9 métodos) e
//!   `brain` — núcleo, não feature tardia — (`brain_*_social_fact`). `character`, `desktop`, `mcp`,
//!   `privacy`, `scheduler`, `user` não registram RPC nenhum hoje — se algum dia registrarem, viram
//!   um quinto caso a cobrir, não um motivo para reabrir os quatro já mapeados.
//!   - `mood`: `engine`/`math`/`tool` existem em Rust, mas o único item exposto por
//!     `features/mood/mod.rs` é `register_tools` — sem despacho por método (`mood_get` etc.) e sem
//!     qualquer menção a RPC no diretório (grep confere). O motor existe; o fio que o liga ao canal
//!     `mika:plugin-rpc` não.
//!   - `gamer-companion`: `steam_art`/`steam_library`/`steam_local_stats` cobrem parte de
//!     `steam:top-games`/`steam:game-art`, mas `session-list`/`session-current`/`mini-game:*` pedem
//!     sqlite de sessão (fora do escopo de um módulo), e o próprio `gamer/mod.rs` já roda o teste
//!     `no_invoke_channels_claimed_yet` — não reivindica nenhum canal de invoke, só tools.
//!   - `person-catalog`: nada em Rust tem esse nome; `chat/persona_composer.rs`/
//!     `persona_relationship.rs` são para montar prompt, não o CRUD que `person_catalog_*` expõe.
//!   - `brain` (fatos sociais): `modules/brain/` em Rust tem `guards.rs`/`reflection.rs`/`turn.rs` —
//!     nenhum símbolo de fato social.
//!   Reivindicar hoje devolveria `HandlerMissing`/erro para TODO método RPC vivo, nos 4 plugins que
//!   de fato usam o canal — pior que o sidecar, que responde certo.
//!
//! Conclusão igual à leitura anterior: nenhum dos dois é reivindicável. `modules()` segue vazio.
//!
//! ## Reconfirmação (2026-09-22) — achado decisivo em `channel_gate.rs`
//!
//! A leitura acima ainda enquadrava o bloqueio como "é código, não dado para portar". Há um motivo
//! mais forte, estrutural: `bridge.rs` roteia por CANAL INTEIRO (`state.native.owns(&channel)`),
//! nunca por método/pluginId — reivindicar qualquer um dos dois canais hoje, mesmo respondendo
//! nomeado para o que falta, corta o sidecar Node da rota inteira e regride os 25 métodos reais de
//! `plugin-rpc` (4 plugins) e os 8 `testConnection` reais de `plugin-test` (a leitura anterior
//! especulava "pode ser zero" — está errada, é 8 de 9). Detalhe e testes: `channel_gate.rs`.
//!
//! ## E o que isso quer dizer, agora que o objetivo é apagar o sidecar (2026-09-22)
//!
//! "Não é reivindicável hoje" continua verdade e não é o fim da frase. Enquanto estes dois canais
//! descerem para o Node, o sidecar inteiro fica vivo — logo os 25 métodos e os 8 `testConnection`
//! não são um argumento para parar, são a LISTA DE TRABALHO, e os quatro donos dos 25 já estão
//! portados (`person_catalog`, `gamer`, `mood`, `memory/social`).
//!
//! `rpc.rs` é o esqueleto dessa lista: registro `pluginId → método → handler`, guarda de superfície
//! e identidade, e os códigos de erro do contrato — tudo pronto e **sem reivindicar nada**.
//! Cada feature portada registra os próprios métodos; quando o registro alcançar 25, o canal cai.
//!
//! A saída proposta de "reivindicar e reencaminhar ao sidecar o que falta" foi recusada de
//! propósito: ela move a rota sem matar o processo, e é o processo que o objetivo manda apagar.
//!
//! ## `mika:plugin-rpc` CAIU — 2026-09-22 (`native.rs`)
//!
//! A lista de trabalho acima foi executada no mesmo dia: os quatro donos dos 25 métodos ganharam
//! despacho nativo (`features/person_catalog/rpc.rs` 9, `features/mood/rpc.rs` 4, `gamer/rpc.rs` 7,
//! `memory/social/rpc.rs` 4). **24 dos 25 despacham de verdade.**
//!
//! O último, `person_catalog_image`, ganhou handler em 2026-09-28 (`features/person_catalog/image.rs`,
//! sobre `browser/guarded_fetch/`): 25 de 25.
//!
//! **O que estava errado na leitura anterior não era a análise, era o denominador.** "Reivindicar
//! devolveria `handler-missing` para todo método vivo" era verdade enquanto o registro estava
//! vazio — e a frase sobreviveu à hora em que deixou de ser. A regra que fica: recusa baseada em
//! contagem envelhece junto com a contagem, então ela mede de novo antes de valer, e é por isso que
//! `native_tests.rs` cobra os 25 contra o fonte do TS a cada rodada em vez de confiar nesta prosa.
//!
//! `mika:plugin-test` continua no sidecar. Os bloqueios dele são outros e estão medidos abaixo
//! (provedores de LLM/STT/TTS não portados, mais os 8 `testConnection` genéricos), não o
//! denominador do RPC.
use crate::native::NativeModule;

mod native;
pub mod rpc;

#[cfg(test)]
mod channel_gate;

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(native::PluginRpcModule)]
}

#[cfg(test)]
mod tests {
    /// Era `claims_no_channels_yet`, e a mudança é a da rodada inteira: `mika:plugin-rpc` caiu em
    /// 2026-09-22, `mika:plugin-test` não. Continua sendo impossível "vazar" uma reivindicação sem
    /// passar por aqui — só que agora o teste diz QUAL canal é legítimo, em vez de que nenhum é.
    #[test]
    fn claims_plugin_rpc_only() {
        let claimed: Vec<&'static str> = super::modules().iter().flat_map(|module| module.channels().iter().copied()).collect();
        assert_eq!(claimed, vec!["mika:plugin-rpc"]);
    }

    // `plugin-rpc-registry.ts` e `plugin-connection-test.ts` foram removidos (backend Node
    // deletado na migração para Tauri). As duas investigações que estes testes ancoravam —
    // "o RPC despacha para closure arbitrário", "a fronteira barra plugin-view fora do próprio
    // pluginId", "o plugin-test chama LLM/voz/testConnection real" — descreviam a FORMA do lado
    // TS que orientou o porte; o comportamento Rust equivalente já é exercido diretamente por
    // `rpc_tests.rs::a_plugin_view_can_only_call_its_own_plugin`/
    // `a_plugin_view_cannot_reach_another_plugins_methods` (fronteira) e pelos branches reais de
    // `plugin_test.rs`/`llm_provider.rs` (chamada viva), sem precisar mais comparar contra o TS.

    // `ipc/tool-access.ts` foi removido (backend Node deletado na migração para Tauri) — o mesmo
    // invariante ("tools-list-by-plugin filtra o mesmo registro que tools-list") já foi anotado em
    // `modules/tools/tests.rs`, sem duplicar aqui.

    // --- Reconfirmação 2026-09-21d ---------------------------------------------------------

    /// Outra sentinela de premissa que caiu em 2026-09-22, e a mais cara delas: afirmava que o
    /// caminho de LLM em Rust era fixo em Ollama e mandava reler antes de reivindicar
    /// `mika:plugin-test`. `brain/llm_provider.rs` portou `resolveLanguageModelForProvider` sobre o
    /// `rig-core`, e `chat/send.rs` passou a chamar a fachada multi-provedor — a sentinela ficou
    /// vermelha exatamente onde devia.
    ///
    /// Virou a asserção oposta, e ela mede o que importa: os dois caminhos — a CONVERSA e o BOTÃO
    /// que testa a conversa — têm de resolver o provedor pela MESMA fachada. Quando divergem, o
    /// teste de conexão dá verde num provedor que o chat não usa, e foi assim que o botão de STT
    /// ficou mentindo nesta mesma rodada.
    #[test]
    fn the_chat_path_and_the_connection_test_resolve_the_provider_through_the_same_facade() {
        // `plugin-connection-test.ts` foi removido (backend Node deletado na migração para
        // Tauri) — o que importa é os DOIS lados Rust convergirem na mesma fachada, provado abaixo
        // sem comparação contra o TS.
        let send_source = include_str!("../chat/send.rs");
        assert!(
            send_source.contains("run_turn_llm"),
            "chat/send.rs voltou a chamar um caminho fixo de provedor — a conversa regrediu para Ollama-only"
        );

        let test_source = include_str!("../settings/plugin_test.rs");
        let production = test_source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(
            production.contains("run_turn_llm_raw"),
            "settings/plugin_test.rs parou de usar a fachada multi-provedor — o botão Validar volta a \
             medir um provedor diferente do que a conversa usa"
        );
    }

    // `features/mood/index.ts` foi removido (backend Node deletado na migração para Tauri). O
    // invariante "os 4 métodos de mood (mood_get, mood_update, affinity_get,
    // affinity_add_interaction) estão registrados em Rust" já é coberto pelo snapshot congelado em
    // `native_tests::FROZEN_TS_METHODS`/`every_frozen_ts_method_is_registered_in_rust`, que inclui
    // os 4 e lê o mesmo `features/mood/rpc.rs` — duplicar aqui não prova nada a mais.

    /// `mika.voice` em `mika:plugin-test` precisa de STT E de qualquer um dos 7 provedores de TTS.
    /// Supertonic, os 4 remotos e Piper chegaram em Rust; só Kokoro segue corte NOMEADO
    /// (`CUT_PROVIDERS`, ver o cabeçalho de `dispatch.rs`), e o branch STT não tem nenhum
    /// equivalente. Se `CUT_PROVIDERS` encolher/crescer de novo ou um teste de STT aparecer em
    /// `voice/`, a premissa mudou — releia antes de reivindicar.
    #[test]
    fn plugin_test_voice_branch_still_cuts_exactly_the_one_unported_tts_provider() {
        let dispatch_source = include_str!("../voice/tts/dispatch.rs");

        // Do `=` em diante, NÃO do primeiro `[`: a ANOTAÇÃO DE TIPO é `&[&str]`, e cortar no
        // primeiro colchete leria `&str` como se fosse um provedor cortado. Quarta vez que esta
        // armadilha aparece nesta árvore — a anotação quase sempre tem colchete antes do literal.
        let start = dispatch_source.find("const CUT_PROVIDERS").expect("CUT_PROVIDERS sumiu de dispatch.rs");
        let list = &dispatch_source[start..];
        let list = &list[list.find('=').expect("atribuição de CUT_PROVIDERS")..];
        let list = &list[list.find('[').expect("abertura da lista")..list.find(']').expect("fim da lista")];
        let cut: Vec<&str> =
            list.split(',').map(|item| item.trim().trim_matches(['[', '"', ' '])).filter(|item| !item.is_empty()).collect();

        assert_eq!(
            cut,
            vec!["kokoro"],
            "CUT_PROVIDERS mudou — se ENCOLHEU, um provedor de TTS virou real e o branch mika.voice \
             de mika:plugin-test melhorou; se CRESCEU, uma feature regrediu. Os dois casos pedem releitura"
        );

        // A versão anterior deste teste perguntava se cada literal aparecia EM ALGUM LUGAR do
        // arquivo, e continuou VERDE quando os quatro provedores remotos foram portados: os braços
        // novos do `match` contêm os mesmos literais. Gate de substring mede o texto, não a decisão.
        for ported in ["elevenlabs", "openai-tts", "azure-tts", "google-tts", "piper", "system"] {
            assert!(!cut.contains(&ported), "`{ported}` voltou para CUT_PROVIDERS — o provedor regrediu");
        }
    }

    /// O outro lado do mesmo par: o branch STT do `mika.voice`. Era uma asserção NEGATIVA em
    /// `voice/mod.rs` ("não existe nada parecido com teste de STT"), e ela também ficou verde
    /// depois do porte — `pub(crate) mod stt_cloud;` não contém nenhuma das substrings que ela
    /// procurava. Duas premissas falsas, dois gates verdes, pelo mesmo motivo.
    #[test]
    fn plugin_test_voice_branch_has_real_cloud_stt() {
        let source = include_str!("../voice/stt_cloud.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        for needle in ["\"groq-whisper\"", "\"openai-whisper\"", "pub(crate) async fn validate_credentials"] {
            assert!(
                production.contains(needle),
                "voice/stt_cloud.rs perdeu `{needle}` — o STT em nuvem regrediu e a tela de Configurações \
                 volta a dizer que ele não existe"
            );
        }
    }
}
