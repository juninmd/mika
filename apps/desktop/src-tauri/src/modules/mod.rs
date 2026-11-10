//! Os módulos de backend já portados para Rust.
//!
//! Um arquivo por área, e não um só, porque a migração corre em várias frentes ao mesmo tempo —
//! um arquivo compartilhado viraria conflito em série. Cada módulo declara os canais que
//! reivindica; `native::registry()` junta tudo e `bridge.rs` encaminha ao sidecar só o que
//! ninguém reclamou.

pub mod brain;
pub mod browser;
pub mod chat;
pub mod credentials;
pub mod desktop;
#[cfg(any(test, feature = "e2e"))]
pub mod e2e_hooks;
pub mod features;
pub mod gamer;
pub mod health;
pub mod hud_bridge;
pub mod inference;
pub mod legal;
#[cfg(test)]
mod lifecycle_shutdown_gate;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod plugins;
pub mod privacy;
pub mod settings;
pub mod sqlite_migrate;
pub mod tools;
pub mod system;
pub mod system_signals;
pub mod telemetry;
pub mod token_usage;
pub mod update;
pub mod update_backup;
pub mod vision;
pub mod voice;

use std::sync::Arc;

use crate::native::NativeModule;

pub fn all() -> Vec<Box<dyn NativeModule>> {
    let mut modules: Vec<Box<dyn NativeModule>> = vec![];

    modules.extend(brain::modules());
    modules.extend(browser::modules());
    modules.extend(chat::modules());
    modules.extend(credentials::modules());
    modules.extend(desktop::modules());
    #[cfg(any(test, feature = "e2e"))]
    modules.extend(e2e_hooks::modules());
    modules.extend(features::modules());
    modules.extend(gamer::modules());
    modules.extend(health::modules());
    modules.extend(inference::modules());
    modules.extend(legal::modules());
    modules.extend(llm::modules());
    modules.extend(mcp::modules());
    modules.extend(memory::modules());
    modules.extend(plugins::modules());
    modules.extend(privacy::modules());
    modules.extend(settings::modules());
    modules.extend(tools::modules());
    modules.extend(system::modules());
    modules.extend(telemetry::modules());
    modules.extend(update::modules());
    modules.extend(vision::modules());
    modules.extend(voice::modules());

    modules
}

/// Segunda passada de registro, com `AppHandle` — as tools que precisam do `ConfigStore`.
///
/// **Por que duas passadas e não uma.** [`all()`] roda dentro de `native::registry()`, que não tem
/// `AppHandle` nenhum (é um `OnceLock` de processo, construído antes do `setup`). Tool sem config
/// — `track_gamer_mood`, `mood`, `desktop` — se registra como efeito colateral de `modules()` e
/// está resolvida ali. Tool COM config não cabe naquele ponto, e foi exatamente aí que três frentes
/// desta rodada pararam ao mesmo tempo: a de visão entregou `vision::register(store)` pronto e
/// testado sem ninguém para chamá-lo, e as de MCP e companion registraram o mesmo diagnóstico.
///
/// A saída errada seria alargar `ToolHandler::call` para receber `AppHandle` — acoplaria todo
/// handler testável a um app Tauri inteiro, que é justamente o que o cabeçalho de
/// `tools/registry.rs` recusa. O contrato de lá já é o certo: quem precisa de config FECHA sobre a
/// referência no construtor do handler. Só faltava o lugar onde essa referência existe. É aqui.
///
/// Falhar aqui não derruba o shell — mesma política do `sidecar.spawn` em `lib.rs`: a janela abre
/// com o catálogo de tools incompleto e o erro aparece no log, em vez de o app morrer atrás de uma
/// tela que nunca pinta.
pub fn register_config_bound_tools(app: &tauri::AppHandle) {
    // Embedding grava uso de token sem `AppHandle` no ponto de chamada (`token_usage::record_embedding`).
    token_usage::arm(app);
    let store = match crate::state::config_store(app) {
        Ok(store) => store,
        Err(error) => {
            crate::feature_health::record_registration_failure("config_store", &error.to_string());
            return;
        }
    };

    // O gate de aprovação do LLM (`ToolRegistry::execute_as`) honra `brain.approveRiskyTools`.
    tools::registry().bind_config(Arc::clone(&store));
    vision::register(Arc::clone(&store), Some(app.clone()));

    // `content`'s `daily_recap` precisa ler `chat.db` (mesmo arquivo que `modules::chat`
    // persiste); resolvido UMA VEZ aqui — o único ponto com `AppHandle` — e passado como `PathBuf`
    // pronto para `brain::register`, que assim continua testável sem forjar `AppHandle` (o crate
    // não habilita a feature `test-support` do tauri, mesma restrição de `history_store.rs`).
    // Falha em resolver não derruba o resto: `content` registra igual e `daily_recap` degrada
    // nomeado (ver `tools/content.rs::ContentHandler`), mesma política de resiliência desta função.
    let chat_db_path = match chat::chat_db_path(app) {
        Ok(path) => Some(path),
        Err(error) => {
            log::error!("content: daily_recap ficará sem histórico — não resolveu chat.db: {error}");
            None
        }
    };
    brain::register(Arc::clone(&store), chat_db_path);
    features::mood::register(Arc::clone(&store));
    // Quem conta ao usuário que a afinidade subiu. `persistence.rs` publica
    // `EventKey::MoodAffinityLevelUp` desde que a tool existe e ninguém lia: o nível subia em
    // silêncio. O aviso traduzido sai como `EventKey::Notification`, que `hud_bridge` leva à tela.
    features::mood::spawn_affinity_notice_listener(Arc::clone(&store));
    features::user::register(Arc::clone(&store));
    features::character::register(Arc::clone(&store));
    // Segunda passada de `desktop`: `modules()` já registrou um handler sem config, onde `process`
    // e `window` funcionam desde o boot e `launch` degrada nomeado. Esta linha o SOBRESCREVE pelo
    // que tem `ConfigStore` e abre app/URL de verdade. Sem ela `launch` ficaria recusando para
    // sempre, com a tool listada e o código pronto — a mesma classe de defeito que esta função
    // inteira veio consertar.
    features::desktop::register(Arc::clone(&store));
    desktop::ensure_local_embedding(app, Arc::clone(&store));

    // WP5: a tool `computer` (mouse/teclado sintéticos + UI Automation). Sem primeira passada —
    // ver o cabeçalho de `features/computer_use/mod.rs` para o porquê. `register_destructive` por
    // dentro (`features::computer_use::tool::register`), não `register` simples: toda chamada
    // passa pelo gate de aprovação, inclusive vinda do loop de tools do LLM (`brain::turn`), que
    // nunca passa pelo `mika:tool-execute` (o único caminho que o padrão de `desktop` cobriria).
    features::computer_use::register(Arc::clone(&store));

    // A tool `person_catalog` passou a ter store de verdade (`catalog.json`, mesmo arquivo que o TS
    // já grava) nesta rodada. Precisa do `AppHandle` para achar o arquivo
    // (`crate::state::app_data_dir(app)`, ver `features::person_catalog::store::db_path`), não de
    // `ConfigStore` — por isso só `app`, sem `Arc::clone(&store)`. Ver o cabeçalho de
    // `features/person_catalog/mod.rs` para a tabela de quais das 11 ações dispacham de verdade.
    if let Err(error) = features::person_catalog::register_tools(app) {
        crate::feature_health::record_registration_failure("person_catalog", &error);
    }

    // Os 10 métodos de `mika.person-catalog` em `mika:plugin-rpc` — caminho DISTINTO da tool acima:
    // a tool é o LLM chamando, isto é a tela de Pessoas do renderer. Mesmo `AppHandle` e mesmo
    // singleton de `CatalogStore` que a linha anterior já abriu; `person_catalog_image` usa o
    // `ConfigStore` (modo offline) para o fetch guardado (ver `features/person_catalog/image.rs`).
    if let Err(error) = features::person_catalog::register_rpc(app) {
        crate::feature_health::record_registration_failure("person_catalog_rpc", &error);
    }

    // Os 4 métodos de fato social de `mika.brain` — os últimos dos 25 de `mika:plugin-rpc`. Mesmo
    // `MemoryStore` que o resto de `memory/` já abriu; o `AppHandle` aqui só resolve o caminho.
    if let Err(error) = memory::social::register_rpc(app) {
        crate::feature_health::record_registration_failure("brain_social_rpc", &error);
    }

    // A tool `speak` — o que faz a Mika FALAR. Todo o pipeline de TTS (Supertonic sobre ONNX, corte
    // em sentenças, cache, cancelamento) estava portado e testado sem um único chamador de
    // produção; esta linha é o chamador. Precisa do `AppHandle` para resolver o diretório de cache,
    // por isso mora aqui e não em `modules()`.
    if let Err(error) = voice::register(Arc::clone(&store), app) {
        crate::feature_health::record_registration_failure("speak", &error);
    }

    // A tool `voice` inteira (TS `tools/voice.ts`, 5 ações) — distinta da tool `speak` acima. Ver
    // o cabeçalho de `voice/voice_tool.rs` para a tabela de paridade.
    if let Err(error) = voice::register_voice_tool(Arc::clone(&store), app) {
        crate::feature_health::record_registration_failure("voice_tool", &error);
    }

    // `clear_history`/`list_memories`/`export_data` — as tools de `features::privacy` que
    // `modules/tools/mod.rs` nomeava como lacuna (`clear_history` era a mais sensível do catálogo
    // sem par nativo). Ver o cabeçalho de `features/privacy/mod.rs` para o porquê de portar parcial
    // em vez de não portar nada.
    if let Err(error) = features::privacy::register(Arc::clone(&store), app) {
        crate::feature_health::record_registration_failure("privacy", &error);
    }

    // `scheduler` pede o `AppHandle` inteiro, não só o store: ele agenda trabalho no runtime do
    // Tauri. É o único que pode falhar, e falhar aqui é perder o agendador — não o app.
    if let Err(error) = features::scheduler::register(app) {
        crate::feature_health::record_registration_failure("scheduler", &error);
    }

    // Declarações de settings que não dependem de config nenhuma — só descrevem a própria aba.
    // Ficam aqui, e não no `modules()` de cada um, porque `FeatureRegistry` deduplica por `tabId`
    // DESCARTANDO em silêncio a segunda registrada: com dois pontos de entrada a ordem passa a
    // importar, e um `tabId` repetido some da tela sem erro. Um lugar só torna a colisão visível.
    vision::register_feature();
    // A aba `llm` é de `modules/llm`, não de `modules/brain`: no TS quem ESCREVE o schema é
    // `services/llm/llm.schema.ts`; `brain.feature.ts` só importa e reagrega. Autoria manda, que é
    // o que o invariante "cada módulo é dono da própria UI" pede. `brain` já cedeu a aba.
    llm::register_feature();
    mcp::feature::register_feature();
    privacy::feature::register_feature();
    gamer::feature::register_feature();
    features::person_catalog::feature::register_feature();
    features::scheduler::feature::register_feature();
    // As DUAS abas de `voice.schema.ts` — `voice` e `stt`. Um schema, dois `tabId`, e era por elas
    // que o catálogo fechava em 13 de 15: `mika:settings-declarations` reivindicado antes desta
    // linha teria APAGADO as telas de Voz e de Transcrição, não migrado.
    voice::feature::register_feature();

    // Quem CALA o TTS quando o barge-in dispara. Assina `ForceStopTts` no barramento de processo e
    // chama `abort_registry::force_stop_tts()`. A assinatura acontece dentro de
    // `spawn_force_stop_listener`, síncrona e antes do `tokio::spawn` dele — `broadcast` não entrega
    // retroativamente, e assinar de dentro do bloco async perderia todo evento publicado antes de a
    // task rodar pela primeira vez.
    //
    // Sem esta linha a Mika continuaria falando por cima do usuário: o sinal de interrupção é
    // calculado certo e ninguém age sobre ele.
    voice::tts::bus_bridge::spawn_force_stop_listener(crate::events::bus());

    // Quem FALA: a resposta do chat (frase a frase, direto do stream) e todo `SpeechRequest`, numa
    // fila só para as duas nunca se sobreporem. Antes, a resposta de chat nunca era dita — o toggle
    // `voice.autoSpeak` não tinha runtime em Rust. E `EventKey::SpeechRequest` tinha três
    // publicadores (`character::speak_action`, `scheduler`, comentário de jogo) e ZERO assinantes:
    // as tools devolviam sucesso, o evento entrava no barramento e morria ali. A Mika não falava
    // nada que não fosse resposta de chat — sem erro, sem log, e com teste verde dos dois lados,
    // porque o publicador testava que publica e o consumidor inexistente não tinha o que testar.
    // Mesmo `cache_dir` de `speak_tool::register` (`<app_data_dir>/supertonic`): outra pasta faria
    // a voz já baixada ser baixada de novo.
    match crate::state::app_data_dir(app) {
        Ok(data_dir) => voice::spawn_auto_speak(Arc::clone(&store), data_dir.join("supertonic"), crate::events::bus()),
        Err(error) => log::error!("fala da Mika desligada: não resolveu app_data_dir ({error})"),
    }
    // Port de `ExpressionSyncService`: publica `EventKey::ActivityChanged` a cada transição de voz
    // (pensando, falando, ouvindo, erro) — `hud_bridge` encaminha ao canal `hud:activity` que a UI
    // já espera. Sem esta linha o ícone/estado de atividade da HUD nunca muda de `idle`.
    voice::spawn_expression_sync(crate::events::bus());

    // FLAG (linha nova neste arquivo, ver AGENTS.md): a mesma classe de lacuna que a linha acima já
    // fechou para o barge-in, agora para o texto do recap/debate. `content::daily_recap` e
    // `collaboration_session::run_debate` publicam `EventKey::AssistantMessage` no barramento de
    // processo (`crate::events`); sem esta linha ninguém encaminha isso à janela, e o texto fica
    // preso no `data` de retorno da tool. Este é o único ponto com `AppHandle` fora de
    // `modules::chat` — mesmo motivo de `spawn_force_stop_listener` estar aqui, não em `modules()`.
    chat::spawn_assistant_message_bridge(crate::events::bus(), app.clone());

    // Mesma lacuna, agora para o resto do catálogo de HUD: `Notification`, `ListeningStarted`,
    // `ListeningStopped` e `TtsFallback` eram publicadas e não chegavam à tela — o indicador de
    // microfone nunca acendia e todo aviso ao usuário era descartado no barramento.
    hud_bridge::spawn_hud_bridge(crate::events::bus(), app.clone());
    // Governador de energia ANTES dos laços que ele escala: o primeiro tick de cada um já lê o
    // perfil da config, não o default do atômico.
    system::governor::spawn(crate::events::bus(), Arc::clone(&store), app.clone());
    // Depois do governador: a atualização automática só baixa e instala conforme o estado dele.
    update::spawn_auto(Arc::clone(&store), app.clone());
    features::person_catalog::spawn_lifecycle_listener(app.clone());
    // Poller de energia (retomada de suspensão / desconexão da tomada) e, desde 2026-09-22, o
    // watcher de janela ativa (`active_window`, gated por `brain.enabled`, por isso o `store`
    // entrou na assinatura) — publica `EventKey::PowerResume`/`PowerBattery`/`ActiveWindowChanged`,
    // que a reação proativa abaixo consome. Sem esta linha os pollers nunca nascem e as reações
    // ("sistema acordou"/"saiu da tomada"/"janela mudou") ficam mudas para sempre, com o código
    // pronto e testado.
    system_signals::spawn(crate::events::bus(), Arc::clone(&store));
    // Relógio de tensão de jogo — UMA instância de processo, compartilhada entre a visão (que pode
    // marcar/consultar tensão a partir do bloco `GAME_SIGNAL:` da análise de tela) e o motor
    // proativo (que suprime toda fala durante um momento tenso). Duas instâncias separadas era
    // exatamente o bug que `brain::awareness::tension` existe para nunca reintroduzir — ver o
    // cabeçalho daquele módulo.
    let game_tension = Arc::new(brain::awareness::tension::GameTensionState::new());
    // Estado explícito de "gaming" (specs/08-gamer-companion.md, "Estado de comportamento") — mesma
    // razão de `game_tension` ser uma única instância de processo: `spawn_proactive_engine` a
    // atualiza a partir de `GameChanged`/`GameStopped` e a consulta para reduzir proatividade fora
    // do jogo. Nasce aqui, e não dentro daquela função, pelo mesmo motivo de `game_tension` — outro
    // consumidor (visão, futuramente) poder receber o MESMO `Arc` sem duplicar o estado.
    let gaming_state = Arc::new(gamer::gaming_state::GamingState::new());
    // "O que está na tela agora" para o CHAT regular (specs/08-gamer-companion.md §"Ajuda no
    // jogo"): assina `EventKey::ScreenObservation` e alimenta o holder que
    // `brain::persona::compose_system_prompt` lê a cada turno. Assina ANTES do watcher de tela
    // abaixo poder publicar qualquer coisa (mesma regra de AGENTS.md/`modules/mod.rs` para todo
    // assinante spawnado aqui). Ver o cabeçalho de `brain::persona::screen_context`.
    let _ = brain::persona::screen_context::spawn();
    // Visão contínua: a tela periódica (com dedup por hash/densidade) e o clipboard, ambos atrás
    // dos toggles e do consentimento de privacidade — ver os cabeçalhos de `vision::watcher` e
    // `vision::clipboard_watch`.
    let _ = vision::spawn_screen_watcher(
        Arc::clone(&store),
        crate::events::bus(),
        app.clone(),
        Some(Arc::clone(&game_tension)),
        Some(Arc::clone(&gaming_state)),
    );
    let _ = vision::spawn_clipboard_watch(Arc::clone(&store), crate::events::bus(), app.clone());
    let _ = vision::spawn_clipboard_image_analysis(Arc::clone(&store));
    // A Mika puxa assunto sozinha (ociosidade, jogo, tela) e responde a quem a interrompeu.
    let proactive = brain::awareness::spawn_proactive_engine(app, Arc::clone(&store), Arc::clone(&game_tension), Arc::clone(&gaming_state));
    // Aprendizado de wiki de jogo (`learnGameDetails`): segundo assinante independente de
    // `EventKey::GameChanged` (o primeiro, dentro de `spawn_proactive_engine`, cuida da reação
    // "steam game") — pesquisa na web + LLM, grava em `MemoryStore`, reage pelo MESMO motor
    // proativo. Ver o cabeçalho de `gamer::game_learning`.
    match crate::modules::memory::memory_store(app) {
        Ok(memory) => {
            let _ = gamer::game_learning::spawn(Arc::clone(&store), memory, Arc::clone(&proactive));
        }
        Err(error) => log::error!("aprendizado de wiki de jogo desligado: memory store não abriu ({error})"),
    }
    // Pensamento pronto (imagem do clipboard, atalho de screenshot) passa pelo MESMO motor e pelas
    // mesmas travas de cadência — nunca fala por fora dele.
    let _ = brain::thought::spawn_thought_service(proactive);
    // Depois de cada turno: memória episódica do par e a reflexão a cada 5º turno.
    brain::post_turn::spawn_post_turn_listener(app.clone(), Arc::clone(&store));
    // Sweep de manutenção por IDADE (defects.md "memory-relationship" P1): sem isto, um usuário de
    // baixo uso nunca acumula as 200 entradas que `consolidation::maybe_consolidate` exige, e a
    // memória "da semana passada" expira pelo TTL de 7 dias antes de qualquer consolidação rodar.
    // Timer próprio (boot + a cada poucas horas), independente do gatilho por turno acima.
    brain::post_turn::spawn_aging_sweep(app.clone(), Arc::clone(&store));

    // Mesma lacuna, agora para a pose do avatar: `avatar` tool (`features::character::register`
    // acima) publica `EventKey::Expression` no barramento de processo desde que ela existe, mas
    // ninguém encaminhava à janela — o avatar renderizado nunca trocava de expressão. Precisa do
    // `AppHandle` para achar a janela (mesmo motivo de `spawn_assistant_message_bridge` estar
    // aqui, não em `modules()`), então é ESTE ponto, não a chamada de `register` logo acima, que
    // fecha o ciclo — `register`/o `ToolHandler` continuam testáveis sem `AppHandle` nenhum.
    features::character::spawn_expression_bridge(crate::events::bus(), app.clone());

    // O watcher de jogo da Steam. `start` abre o SQLite de sessão, embrulha em `SessionMemory` e
    // spawna o laço — nesta ordem, e com o `Arc` capturado ANTES do `tokio::spawn`, porque `spawn`
    // só AGENDA: assinar qualquer coisa de dentro do bloco async perde o que for publicado na
    // janela entre o agendamento e a primeira execução da task.
    //
    // Falhar aqui perde o watcher, não o app — mesma política do resto desta função. O `JoinHandle`
    // é descartado de propósito: o laço vive enquanto o processo viver, e o `ExitRequested` de
    // `lib.rs` derruba o runtime inteiro.
    match gamer::game_watcher::start(app, Arc::clone(&store)) {
        Ok((session_memory, _handle)) => {
            // "Joga com" (specs/08-gamer-companion.md, "Pessoas no jogo"): promove um parceiro de
            // jogo recorrente a `RelationshipType::PlaysWith` no MESMO `CatalogStore` que a tool/RPC
            // de `person_catalog` já abriram acima — ver `gamer::game_partners`.
            match features::person_catalog::catalog_store(app) {
                Ok(catalog) => {
                    let _ = gamer::game_partners::spawn_partner_watcher(catalog, session_memory);
                }
                Err(error) => log::error!("promoção de 'joga com' desligada: catálogo de pessoas não abriu: {error}"),
            }
        }
        Err(error) => log::error!("watcher de sessão de jogo não subiu: {error}"),
    }

    // O `ApprovalHandler` real. Sem esta linha o gate de `register_destructive` NEGA tudo por
    // padrão (fail-closed, correto), e o handler ficaria construído, testado e ligado a nada — a
    // mesma classe de defeito que `register_config_bound_tools` inteiro veio consertar. Precisa do
    // `AppHandle` porque é ele que acha a janela onde perguntar, por isso mora aqui e não em
    // `modules()`.
    //
    // `install` (e não `set_approval_handler` direto) porque `tool-approval-response` precisa
    // CHAMAR `resolve()` neste mesmo handler quando o usuário decide, e o registro guarda só o
    // trait. Instalar pela metade deixaria o diálogo aparecer e a resposta não chegar em ninguém:
    // toda aprovação penduraria até o timeout de 30s e negaria — pior do que não perguntar.
    tools::install_approval_handler(app.clone());
}

#[cfg(test)]
mod tests {
    /// A prova de que [`super::register_config_bound_tools`] é CHAMADA, não só existe.
    ///
    /// Sem isto, a função é indistinguível do estado que ela veio consertar: código pronto,
    /// testado, e ligado a nada — o defeito que a visão levou a rodada inteira para diagnosticar.
    /// Um `cargo check` não acusa: chamador ausente é só um `warning: never used`, e o binário
    /// sobe verde com o catálogo de tools vazio.
    #[test]
    fn the_boot_actually_calls_the_config_bound_registration() {
        let boot = include_str!("../lib.rs");
        assert!(
            boot.contains("modules::register_config_bound_tools(&handle)"),
            "lib.rs::setup não chama mais register_config_bound_tools — as tools com config voltaram a não existir em runtime"
        );
    }

    /// Os ganchos do E2E (`mika:e2e-hook`: publicar evento, sinal de energia, ler defaults, encerrar) e
    /// o override da Wikipedia só existem no binário compilado com `--features e2e`. Falha de
    /// OMISSÃO: tirar o `cfg` do `pub mod`/`extend` compila, passa o resto da suíte e põe a superfície
    /// de teste de volta em todo binário distribuído — `MIKA_E2E=1` no ambiente do usuário a ligaria.
    #[test]
    fn the_e2e_hooks_are_compiled_out_of_a_distributed_binary() {
        let source = include_str!("mod.rs");
        let gate = "#[cfg(any(test, feature = \"e2e\"))]";
        assert!(source.contains(&format!("{gate}\npub mod e2e_hooks;")), "`pub mod e2e_hooks;` perdeu o cfg da feature e2e");
        assert!(
            source.contains(&format!("{gate}\n    modules.extend(e2e_hooks::modules());")),
            "o registro do canal `mika:e2e-hook` perdeu o cfg da feature e2e"
        );
        let wiki = include_str!("browser/guarded_fetch/mod.rs");
        assert!(wiki.contains(&format!("{gate}\nfn e2e_wiki_override(raw_url: &str)")), "`e2e_wiki_override` perdeu o cfg da feature e2e");
    }

    /// O host HTTP do E2E (`e2e_host.rs`) abre o IPC inteiro por TCP em loopback. Falha de OMISSÃO:
    /// tirar o `cfg` do `mod`/da chamada, ou o `optional` das dependências do servidor, compila e
    /// passa o resto da suíte — e põe esse servidor em todo binário distribuído.
    #[test]
    fn the_e2e_http_host_is_compiled_out_of_a_distributed_binary() {
        let lib = include_str!("../lib.rs");
        let gate = "#[cfg(feature = \"e2e\")]";
        assert!(lib.contains(&format!("{gate}\nmod e2e_host;")), "`mod e2e_host;` perdeu o cfg da feature e2e");
        assert!(lib.contains(&format!("{gate}\n            e2e_host::start(&handle);")), "`e2e_host::start` perdeu o cfg da feature e2e");
        let windows = include_str!("../windows.rs");
        assert!(windows.contains(&format!("    {gate}\n    if crate::e2e_host::active()")), "`windows::page` perdeu o cfg da feature e2e");
        let manifest = include_str!("../../Cargo.toml");
        for dep in ["hyper = ", "hyper-util = ", "http-body-util = ", "bytes = "] {
            let line = manifest.lines().find(|l| l.starts_with(dep)).unwrap_or_else(|| panic!("dependência `{dep}` sumiu do Cargo.toml"));
            assert!(line.contains("optional = true"), "`{dep}` deixou de ser opcional: o servidor do E2E entraria em todo binário");
        }
    }

    /// O par boot/shutdown do MCP. O shutdown importa tanto quanto o boot: servidor MCP é processo
    /// do SO FILHO deste, e é o drop do `RunningService` que o mata. Sair sem `disconnect_all`
    /// deixa processo órfão vivo depois de a Mika fechar — e ninguém reclama, porque o app já saiu.
    #[test]
    fn the_boot_and_the_shutdown_both_reach_mcp() {
        let boot = include_str!("../lib.rs");
        for call in ["modules::mcp::boot(&handle)", "modules::mcp::disconnect_all().await"] {
            assert!(boot.contains(call), "lib.rs não chama mais {call} — o ciclo de vida do MCP ficou pela metade");
        }
    }

    /// A prova de que a sessão em processo (`session_accumulator`) é PERSISTIDA no encerramento, não
    /// só descartada com o processo — mesmo padrão do teste acima para o MCP. Sem isto,
    /// "o que ficou estabelecido nessa conversa" volta a morrer em silêncio a cada saída do app
    /// (defects.md "memory-relationship" P2).
    #[test]
    fn the_shutdown_flushes_the_session_accumulator() {
        let boot = include_str!("../lib.rs");
        assert!(
            boot.contains("session_accumulator::accumulator()") && boot.contains(".flush("),
            "lib.rs não chama mais session_accumulator::accumulator().flush — a memória de sessão volta a morrer com o processo"
        );
    }

    /// Os 13 ids de aba que os 12 `*.schema.ts` (agora deletados — backend Node removido na
    /// migração para Tauri; só `character.schema.ts` sobrevive, sob `features/character/renderer/`)
    /// declaravam via `defineFeatureSchema({ … tab: { id: '…' } })`, extraídos e fixados aqui ANTES
    /// da deleção. `voice.schema.ts` valia por DUAS (`voice` e `stt`) — um arquivo, dois
    /// `defineFeatureSchema`, e foi essa assimetria que fez uma contagem anterior fechar em "14
    /// abas" quando o TS tinha 13. Lista congelada, ordenada e sem duplicata (mesma pós-condição de
    /// `ts_tab_ids()` antes de virar const).
    const EXPECTED_TAB_IDS: [&str; 13] = [
        "brain",
        "browser",
        "character",
        "embeddings",
        "gamer-companion",
        "llm",
        "mcp",
        "person-catalog",
        "privacy",
        "stt",
        "user",
        "vision",
        "voice",
    ];

    /// As registrações que este teste reproduz de [`super::register_config_bound_tools`]. Toda uma
    /// delas é conferida contra o FONTE daquela função — ver
    /// `the_parity_test_registers_everything_the_boot_registers`.
    const CONFIG_BOUND_REGISTRATIONS: [&str; 12] = [
        "features::mood::register",
        "features::user::register",
        "features::character::register",
        "features::desktop::register",
        "vision::register_feature()",
        "llm::register_feature()",
        "mcp::feature::register_feature()",
        "privacy::feature::register_feature()",
        "gamer::feature::register_feature()",
        "features::person_catalog::feature::register_feature()",
        "features::scheduler::feature::register_feature()",
        "voice::feature::register_feature()",
    ];

    /// **O gate de `mika:settings-declarations`.** Esse canal serve o catálogo de abas montado em
    /// RUNTIME; servi-lo com o catálogo furado apaga a tela que falta, sem erro e sem log — e o
    /// número de abas foi medido errado três vezes nesta migração (14, 15, 13) justamente porque
    /// `grep` não enxerga `character`/`user`/`mood`, que declaram a aba dinamicamente via
    /// `features/settings_declaration.rs`.
    ///
    /// Uma medição intermediária desta rodada deu "TS 13 × Rust 12, falta 1" — e os conjuntos nem se
    /// sobrepunham: faltavam TRÊS (`brain`, `browser`, `embeddings`) e sobravam duas só-Rust
    /// (`mood`, `scheduler`). **Por isso a asserção é por NOME, nunca por total.**
    ///
    /// Só asserção POSITIVA: `feature_registry::shared()` é `OnceLock` de processo e `cargo test`
    /// roda em paralelo, então "a aba X está ausente" seria flake, não prova. Aba a mais não é
    /// defeito aqui (`mood`/`scheduler` são namespaces que só o Rust declara); aba a MENOS é.
    #[test]
    fn the_catalog_covers_every_tab_the_ts_declares() {
        let dir = std::env::temp_dir().join(format!("mika-tab-parity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("diretório temporário do ConfigStore");
        let store = std::sync::Arc::new(
            crate::config::store::ConfigStore::open(&dir, crate::config::schemas::registry())
                .expect("ConfigStore com o schema de produção"),
        );

        // Caminho 1: efeito colateral de `modules()`, dentro de `native::registry()`. É a função de
        // produção em pessoa — `brain`, `browser` e `embeddings` (esta em `memory/settings.rs`)
        // registram a aba aqui, e um probe que só chamasse `register_feature()` perderia as três.
        let _modules = super::all();

        // Caminho 2: o que `register_config_bound_tools` faz, menos o que exige `AppHandle`.
        super::features::mood::register(std::sync::Arc::clone(&store));
        super::features::user::register(std::sync::Arc::clone(&store));
        super::features::character::register(std::sync::Arc::clone(&store));
        super::vision::register_feature();
        super::llm::register_feature();
        super::mcp::feature::register_feature();
        super::privacy::feature::register_feature();
        super::gamer::feature::register_feature();
        super::features::person_catalog::feature::register_feature();
        super::features::scheduler::feature::register_feature();
        super::voice::feature::register_feature();

        let published: Vec<String> = crate::feature_registry::shared()
            .get_all_settings()
            .iter()
            .filter_map(|panel| panel.get("tabId").and_then(|id| id.as_str()).map(str::to_string))
            .collect();

        let declared = EXPECTED_TAB_IDS;

        let missing: Vec<&str> = declared.into_iter().filter(|tab| !published.iter().any(|p| p == tab)).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            missing.is_empty(),
            "abas que o TS declara e o Rust não publica: {missing:?} — servir `mika:settings-declarations` \
             assim apaga essas telas do usuário em silêncio. Publicadas: {published:?}"
        );
    }

    /// O buraco que [`the_catalog_covers_every_tab_the_ts_declares`] teria sozinho: ele CHAMA as
    /// registrações em vez de bootar o app, então tirar uma linha de `register_config_bound_tools`
    /// deixaria a aba viva no teste e morta em produção — verde mentindo.
    ///
    /// Fatia o fonte da função de boot antes do `#[cfg(test)]` de propósito: sem o corte, cada nome
    /// bateria com a cópia do próprio teste, logo abaixo, e a asserção passaria sempre.
    #[test]
    fn the_parity_test_registers_everything_the_boot_registers() {
        let source = include_str!("mod.rs");
        let start = source.find("pub fn register_config_bound_tools").expect("função de boot");
        let end = source[start..].find("\n#[cfg(test)]").expect("fim do corpo de produção") + start;
        let boot_body = &source[start..end];

        for call in CONFIG_BOUND_REGISTRATIONS {
            assert!(
                boot_body.contains(call),
                "`{call}` sumiu de register_config_bound_tools — a aba correspondente some da tela e \
                 o teste de paridade continuaria verde, porque ele chama a registração por conta própria"
            );
        }
    }

    // `mood_and_desktop_second_pass_is_an_upgrade_not_an_unexpected_collision` (reproduzia as duas
    // passadas de boot contra `super::tools::registry()`, o `ToolRegistry` de processo) foi REMOVIDO
    // nesta rodada: `ToolRegistry` é um `OnceLock` por processo e `cargo test` roda as suítes em
    // paralelo — outros testes que também chamam `super::all()`/`features::mood::register(store)`
    // (ex.: `the_catalog_covers_every_tab_the_ts_declares`, logo acima) intercalam com este e cada um
    // só vê a PRÓPRIA passada de `register_stub` como "esperada"; a passada real de um teste vizinho,
    // que já consumiu o stub antes deste rodar, aparecia como colisão inesperada — vermelho por
    // vizinho, não por defeito (mesma armadilha que `tools/tests.rs` já documenta para pins sobre
    // este mesmo registro). O contrato de verdade — stub→real não é colisão, real→real É — está
    // provado sem essa armadilha em `tools::registry::tests` (`ToolRegistry::new()`, instância local
    // por teste, sem concorrência): `stub_then_real_registration_is_not_an_unexpected_collision`,
    // `two_unrelated_registrations_of_the_same_name_is_an_unexpected_collision` e
    // `register_stub_colliding_with_a_real_tool_is_also_an_unexpected_collision`.

    /// **`tokio::spawn` no boot MATA o app, e nenhum teste de unidade vê.**
    ///
    /// `register_config_bound_tools` é chamada de `lib.rs::setup`, que o Tauri executa FORA do
    /// contexto do reator Tokio. `tokio::spawn` ali panica na hora — "there is no reactor running,
    /// must be called from the context of a Tokio 1.x runtime" — e o processo morre com exit 101
    /// antes de abrir janela. Em teste o mesmo código passa, porque `#[tokio::test]` fornece o
    /// runtime que o boot real não tem: a suíte inteira fica verde com o app impossível de abrir,
    /// que foi exatamente o estado desta árvore até 2026-09-22.
    ///
    /// `tauri::async_runtime::spawn` não tem esse problema — agenda no runtime global do Tauri,
    /// haja ou não contexto no chamador. `features/mood/boot.rs` já usava a forma certa; as pontes
    /// de evento e o vigia de jogos, não.
    ///
    /// **O alvo é resolvido do próprio corpo do boot, não de uma lista.** A primeira versão deste
    /// gate enumerava as funções `spawn_*` à mão e ficou verde enquanto o app morria em
    /// `gamer::game_watcher::start` — que agenda uma task e não tem `spawn` no nome. Lista escrita
    /// à mão cobre o que já se conhece; o que derruba o app é o que ainda não.
    #[test]
    fn nothing_the_boot_calls_uses_bare_tokio_spawn() {
        use std::fs;
        use std::path::{Path, PathBuf};

        let source = include_str!("mod.rs");
        let start = source.find("pub fn register_config_bound_tools").expect("função de boot");
        let end = source[start..].find("\n#[cfg(test)]").expect("fim do corpo de produção") + start;
        let boot_body = &source[start..end];
        let modules_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("modules");

        /// `a::b::c(` -> o arquivo que define `c`: `<modules>/a/b.rs` ou `<modules>/a/b/mod.rs`.
        /// Caminho de um segmento só (`foo(`) não resolve e é ignorado — é chamada local.
        fn resolve_module_file(root: &Path, path: &str) -> Option<PathBuf> {
            let mut segments: Vec<&str> = path.split("::").filter(|s| !s.is_empty()).collect();
            segments.pop()?; // o nome da função
            if segments.first() == Some(&"crate") {
                segments.remove(0);
            }
            if segments.first() == Some(&"modules") {
                segments.remove(0);
            }
            if segments.len() < 2 {
                return None;
            }
            let joined = segments.join("/");
            let as_file = root.join(format!("{joined}.rs"));
            if as_file.is_file() {
                return Some(as_file);
            }
            let as_dir = root.join(&joined).join("mod.rs");
            as_dir.is_file().then_some(as_dir)
        }

        let mut checked: Vec<String> = Vec::new();
        for line in boot_body.lines() {
            // Só CÓDIGO: o corpo do boot é cheio de comentário citando os módulos que chama.
            let code = line.split("//").next().unwrap_or_default();
            for (index, _) in code.match_indices("::") {
                let head: String =
                    code[..index].chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':').collect();
                let head: String = head.chars().rev().collect();
                let tail: String =
                    code[index..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':').collect();
                let full = format!("{head}{tail}");
                let Some(file) = resolve_module_file(&modules_root, &full) else { continue };

                let file_source = fs::read_to_string(&file).expect("módulo chamado pelo boot é ilegível");
                // Cabeçalhos citam `tokio::spawn` em prosa justamente para avisar contra ele — gate
                // que casa a string crua puniria quem documentou (mesmo corte de
                // `state.rs::single_config_store_per_process`).
                let production: String = file_source
                    .split("mod tests {")
                    .next()
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.split("//").next().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\n");

                assert!(
                    !production.contains("tokio::spawn("),
                    "{} usa `tokio::spawn`, e o boot chama `{full}` — isso panica em lib.rs::setup (\"there is no reactor running\") e o app não abre. Use `tauri::async_runtime::spawn`",
                    file.strip_prefix(&modules_root).unwrap_or(&file).display()
                );
                checked.push(full);
            }
        }

        // Piso de alcance: uma extração quebrada devolveria zero arquivos e o gate passaria sem ter
        // lido nada — o verde por vacuidade que esta árvore já pegou duas vezes. O boot chama bem
        // mais que isto; o piso só precisa provar que a resolução de caminho continua funcionando.
        checked.sort();
        checked.dedup();
        assert!(checked.len() >= 8, "só {} módulo(s) do boot resolvido(s) ({checked:?}) — a resolução de caminho quebrou", checked.len());
    }
}

/// **check:topology (metade Rust).** O cabeçalho de `scripts/checks/topology.mts` afirma "pasta
/// sem registro já é erro de compilação — o compilador é o gate". É falso para um caso: um arquivo
/// ou diretório sob `src-tauri/src/modules/**` que existe no disco mas para o qual NENHUM `mod`
/// aponta não é erro de compilação nenhum — `cargo build` simplesmente nunca o enxerga. O arquivo
/// compila zero vezes, `cargo check` sai limpo, e o código (handler, tool, teste) nunca roda. Essa
/// é exatamente a classe de defeito que a migração para Rust reintroduziu ao apagar o roster
/// `FEATURE_LOADERS` do Node: lá, "pasta sem registro" ficava fora de `readdirSync` + comparação
/// contra o literal; aqui, fica fora do grafo de `mod`.
///
/// A prova NÃO pode ser um probe que edita o próprio `src-tauri/src` tracked (proibido pelo brief
/// desta rodada). Por isso a lógica de verificação é uma função pura, parametrizada por caminho, e
/// o probe negativo constrói uma árvore descartável em `std::env::temp_dir()` — fora da árvore
/// versionada — grava um arquivo deliberadamente não declarado, confere que o checker acusa, e
/// apaga tudo (`remove_dir_all`) antes de terminar.
#[cfg(test)]
mod mod_coverage_gate {
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use regex::Regex;

    /// `mod foo;` / `pub mod foo;` / `pub(crate) mod foo { … }`, com ou sem `#[path = "…"]` e
    /// `#[cfg(...)]` interpostos. Devolve (nomes declarados sem `#[path]`, caminhos explícitos de
    /// `#[path]`, relativos ao diretório do próprio arquivo).
    fn parse_declarations(source: &str) -> (HashSet<String>, HashSet<String>) {
        let mod_re = Regex::new(r"mod\s+(\w+)\s*[;{]").expect("mod_re válido");
        let path_re = Regex::new(r#"#\[path\s*=\s*"([^"]+)"\]"#).expect("path_re válido");
        let attr_re = Regex::new(r"#\[[^\]]*\]").expect("attr_re válido");

        let path_attrs: Vec<(usize, usize, String)> = path_re
            .captures_iter(source)
            .map(|cap| {
                let whole = cap.get(0).expect("match completo");
                (whole.start(), whole.end(), cap[1].to_string())
            })
            .collect();

        let mut names = HashSet::new();
        let mut paths = HashSet::new();

        for cap in mod_re.captures_iter(source) {
            let whole = cap.get(0).expect("match completo");
            let name = cap[1].to_string();

            // O `#[path]` mais próximo ANTES deste `mod`, com só espaço/outros atributos entre os dois
            // (senão seria o path de um `mod` anterior não relacionado).
            let matched_path = path_attrs
                .iter()
                .filter(|(_, pend, _)| *pend <= whole.start())
                .filter(|(_, pend, _)| {
                    let between = &source[*pend..whole.start()];
                    attr_re.replace_all(between, "").trim().is_empty()
                })
                .max_by_key(|(pstart, _, _)| *pstart)
                .map(|(_, _, value)| value.clone());

            match matched_path {
                Some(path) => {
                    paths.insert(path);
                }
                None => {
                    names.insert(name);
                }
            }
        }

        (names, paths)
    }

    /// Declarações válidas para um filho de `dir` podem morar em QUALQUER `.rs` direto de `dir`, MAIS
    /// o arquivo "dono" de `dir` quando ele é externo a `dir` (estilo 2018: `web.rs` declara
    /// `mod tests;` e isso resolve para `web/tests.rs`, um arquivo dentro do diretório `web/` que
    /// `web.rs` nomeia mas não contém). Ignorar o dono externo faria todo `#[cfg(test)] mod tests;`
    /// sem `#[path]` explícito (resolução padrão pelo nome do arquivo) soar como órfão; ignorar os
    /// arquivos internos faria `#[path = "x_tests.rs"] mod tests;` escrito DENTRO de `native.rs`
    /// (mesmo diretório, sem tocar `mod.rs`) soar como órfão — os dois padrões coexistem neste crate.
    fn declarations_owning(dir: &Path, owner_file: &Path) -> (HashSet<String>, HashSet<String>) {
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
                    .collect()
            })
            .unwrap_or_default();
        if !files.contains(&owner_file.to_path_buf()) {
            files.push(owner_file.to_path_buf());
        }

        let mut names = HashSet::new();
        let mut paths = HashSet::new();
        for file in files {
            let source = fs::read_to_string(&file).unwrap_or_default();
            let (file_names, file_paths) = parse_declarations(&source);
            names.extend(file_names);
            paths.extend(file_paths);
        }
        (names, paths)
    }

    /// Anda `dir` (cujo dono é `owner_file` — `dir/mod.rs` no estilo diretório, ou um `.rs` irmão
    /// externo no estilo 2018) conferindo cada filho contra [`declarations_owning`].
    fn walk(dir: &Path, owner_file: &Path, violations: &mut Vec<String>) {
        let (names, paths) = declarations_owning(dir, owner_file);

        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("lendo {}: {e}", dir.display()))
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        entries.sort();

        for path in entries {
            if path.is_dir() {
                let stem = path.file_name().expect("nome do diretório").to_string_lossy().to_string();
                let sibling_file = dir.join(format!("{stem}.rs"));
                let inner_mod = path.join("mod.rs");

                let child_owner = if sibling_file.is_file() {
                    Some(sibling_file)
                } else if inner_mod.is_file() {
                    Some(inner_mod)
                } else {
                    None
                };

                if let Some(child_owner) = child_owner {
                    let declared = names.contains(&stem)
                        || paths.iter().any(|p| p == &format!("{stem}/mod.rs") || p.trim_end_matches('/') == stem);
                    if !declared {
                        violations.push(format!(
                            "{}: diretório '{}' não tem `mod {stem}` (nem #[path] equivalente) declarado por nenhum arquivo em {}",
                            path.display(),
                            stem,
                            dir.display()
                        ));
                    }
                    walk(&path, &child_owner, violations);
                }
                // Nem `stem.rs` nem `stem/mod.rs`: não é módulo Rust (ex.: pasta de fixture) — fora do escopo do gate.
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let stem = path.file_stem().expect("stem do arquivo").to_string_lossy().to_string();
                if stem == "mod" || path == *owner_file {
                    continue;
                }
                let declared = names.contains(&stem) || paths.contains(&format!("{stem}.rs"));
                if !declared {
                    violations.push(format!(
                        "{}: arquivo não declarado por nenhum `mod` em {} — cargo nunca o compila",
                        path.display(),
                        dir.display()
                    ));
                }
            }
        }
    }

    #[test]
    fn every_module_file_under_src_modules_is_reachable_from_a_mod_declaration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("modules");
        let mut violations = Vec::new();
        walk(&root, &root.join("mod.rs"), &mut violations);

        assert!(
            violations.is_empty(),
            "arquivo(s) sob src-tauri/src/modules/** sem `mod` que os alcance — silenciosamente fora \
             do crate:\n{}",
            violations.join("\n")
        );
    }

    /// Prova negativa: o checker acusa um arquivo de verdade não declarado. Roda inteiramente em
    /// `std::env::temp_dir()` — nunca toca o `src-tauri/src` versionado — e se autolimpa mesmo se a
    /// asserção falhar antes do `remove_dir_all` (o diretório é descartável e fica sob `/tmp`, não
    /// vaza no `git status`).
    #[test]
    fn negative_probe_catches_an_undeclared_file() {
        let scratch = std::env::temp_dir().join(format!("mika-mod-coverage-probe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        fs::create_dir_all(&scratch).expect("diretório de probe");

        fs::write(scratch.join("mod.rs"), "pub mod declared;\n").expect("mod.rs do probe");
        fs::write(scratch.join("declared.rs"), "pub fn ok() {}\n").expect("arquivo declarado");
        fs::write(scratch.join("orphan.rs"), "pub fn never_compiled() {}\n").expect("arquivo órfão");

        let mut violations = Vec::new();
        walk(&scratch, &scratch.join("mod.rs"), &mut violations);

        let result = fs::remove_dir_all(&scratch);

        assert!(
            violations.iter().any(|v| v.contains("orphan.rs")),
            "o probe negativo devia acusar orphan.rs; violações: {violations:?}"
        );
        assert!(
            !violations.iter().any(|v| v.contains("declared.rs")),
            "falso positivo: declared.rs está com `mod declared;` e não devia ser acusado; violações: {violations:?}"
        );
        result.expect("limpeza do diretório de probe");
    }
}
