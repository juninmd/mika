//! Pipeline automático do catálogo de pessoas — porte de
//! `apps/desktop/src/features/person-catalog/handlers/person-catalog-lifecycle.ts` (as fatias
//! `USER_MESSAGE`/`PERSON_MENTION_DETECTED`/`PERSON_CREATED`/`TOOL_RESULT`/`GAME_CHANGED`;
//! `PERSON_CATALOG_AUTO_UPSERT` fica de fora — ver "Não portado" abaixo).
//!
//! ## Gatilho: `EventKey::ThinkingStart`, não um evento novo
//!
//! A TS assina `Events.USER_MESSAGE` (`chat:user-message`). Esse `EventKey` não existe no lado
//! Rust, e este diretório não tem permissão para editar `events/keys.rs`. `EventKey::ThinkingStart`
//! (`chat:thinking-start`) já é publicado por `modules/chat/send.rs::turn_event_bus` com
//! `{ "userContent": <mensagem do usuário> }` — o mesmo texto que `USER_MESSAGE` carregaria — então
//! esta task usa o evento que já existe em vez de propor um novo. Reportado como decisão, não gap.
//!
//! ## `EventKey::ToolResult` — payload sem `action`
//!
//! `EventKey::ToolResult`/`send.rs::turn_event_bus` publicam `{"name", "success"}` a partir de
//! `TurnEvent::ToolCallFinished` — sem `data.action`, que é o que a TS usa
//! (`result.data.action`) para restringir a graça de 30s só a `upsert`/`update_social`/
//! `add_mention`. Sem esse campo, [`mention_gate::should_note_person_tool_result`] degrada para
//! "qualquer chamada BEM-SUCEDIDA da tool `person_catalog`" — mais ampla que o TS, nunca mais
//! estreita (pior caso: adia uma extração por até 30s a mais, nunca perde uma catalogação). Ver o
//! cabeçalho daquela função para o porquê.
//!
//! ## O que ESTE arquivo NÃO porta, e por quê
//!
//! - **`PERSON_CATALOG_AUTO_UPSERT`**: no TS é alimentado por outro handler (auto-detecção fora
//!   deste arquivo) que não tem equivalente Rust ainda; sem publicador, não há o que assinar.
//! - **Resumo da Wikipedia no enriquecimento** — ver o cabeçalho de `enrichment.rs`.
//!
//! ## Reliability fix (pacote "people catalog + relationship graph", item 6)
//!
//! Até esta rodada, `EventKey::ThinkingStart` chamava `.await` DIRETO na extração por LLM dentro do
//! loop assinante — um provedor lento (a norma medida em `smoke-baseline.md`: 5.5-13s de
//! time-to-first-token) prendia o loop pelo mesmo tanto, e `sub.recv()` só volta a ser chamado
//! DEPOIS que o `.await` retorna. Um barramento com capacidade finita (`events/bus.rs`) descarta o
//! backlog que se acumula enquanto o assinante está preso — o pacote da tarefa cita milhares de
//! mensagens perdidas assim. A partir desta rodada, [`spawn_lifecycle_listener`] só faz o SÍNCRONO
//! (pré-checagens + registro no `gate`, que preserva ordenação/dedup) inline; a chamada ao LLM e
//! tudo que depende dela viram [`run_extraction_job`], despachada via `tauri::async_runtime::spawn`
//! — o loop volta a `sub.recv()` no próximo tick, não no fim da extração.

use std::sync::Arc;

use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

use super::ingest::ingest_extraction;
use super::mention_extract::{self, ExtractionInput, KNOWN_PEOPLE_HINT_LIMIT};
use super::mention_gate::{should_note_person_tool_result, MentionGate};
use super::store::CatalogStore;
use super::{app_context, enrichment, store_singleton};

/// = default de `person-catalog.schema.ts::maxMentionsPerPerson`.
const DEFAULT_MAX_MENTIONS: usize = 100;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Porte do pré-filtro `/[A-Z][a-z]+/.test(d.text)` em
/// `person-catalog-mention-detect.ts::handleMentionDetected` — evita chamar o LLM para mensagens
/// que obviamente não citam nome próprio nenhum (custo, não correção: um falso negativo aqui só
/// adia a extração para a próxima mensagem que tenha uma palavra capitalizada).
fn has_capitalized_word(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).any(|pair| pair[0].is_uppercase() && pair[1].is_lowercase())
}

/// `person-catalog.enabled` era um toggle mestre sem leitor nenhum (gate `config-schema-parity`
/// listava a lacuna como `GAP:`) — a UI (`SettingsTabHero`) já desenha o controle, mas desligá-lo
/// não mudava nada em runtime. A partir desta rodada, desligar o mestre também desliga a
/// auto-extração de menções (mesma regra de "só `false` explícito desliga" do irmão
/// `autoExtractOnChat`, para não desligar por ausência de config).
fn catalog_master_enabled(store: &ConfigStore) -> bool {
    store.get("person-catalog.enabled").and_then(|v| v.as_bool()) != Some(false)
}

/// Porte de `ctx.config.get<boolean>('person-catalog.autoExtractOnChat') === false` — só desliga
/// com `false` explícito, qualquer outra coisa (ausente, `true`) mantém ligado. Também respeita o
/// mestre da feature (`person-catalog.enabled`): mestre desligado desliga a extração automática
/// mesmo com `autoExtractOnChat=true`.
fn auto_extract_enabled(store: &ConfigStore) -> bool {
    // Memória desligada = a Mika "não registra nada do que você diz" (é o que a UI promete): nem a
    // frase vai ao modelo de extração, nem a pessoa entra no catálogo.
    crate::modules::memory::memory_recording_allowed(store) && catalog_master_enabled(store) && store.get("person-catalog.autoExtractOnChat").and_then(|v| v.as_bool()) != Some(false)
}

/// `person-catalog.webEnrichment`: consentimento para mandar o NOME de quem acabou de ser catalogado
/// à Wikipédia e aos buscadores (`enrichment.rs`). Opt-in: só `true` explícito libera — ausência,
/// default de perfil novo ou mestre do catálogo desligado não consultam a web.
fn web_enrichment_enabled(store: &ConfigStore) -> bool {
    catalog_master_enabled(store) && store.get("person-catalog.webEnrichment").and_then(|v| v.as_bool()) == Some(true)
}

fn max_mentions(store: &ConfigStore) -> usize {
    store.get("person-catalog.maxMentionsPerPerson").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(DEFAULT_MAX_MENTIONS)
}

/// Assina o barramento e sobe a task assinante — chamar UMA VEZ em
/// `modules::register_config_bound_tools` (fora deste diretório; ver a linha de wiring no relatório
/// da tarefa). Assinatura ANTES do spawn (mesmo padrão de
/// `voice::tts::bus_bridge::spawn_force_stop_listener`): `sub.recv()` só vê publicações que
/// aconteçam DEPOIS de `bus().subscribe()` retornar, então assinar dentro da task perderia qualquer
/// `ThinkingStart` publicado entre o agendamento e a primeira execução.
pub fn spawn_lifecycle_listener(app: AppHandle) {
    let mut sub = bus().subscribe();
    // tauri::async_runtime::spawn, nunca tokio::spawn — `modules::mod.rs` tem um gate estrutural
    // (`nothing_the_boot_calls_uses_bare_tokio_spawn`) que reprova qualquer chamador do boot que
    // use `tokio::spawn` diretamente.
    tauri::async_runtime::spawn(async move {
        let mut gate = MentionGate::new();
        let mut current_game: Option<String> = None;
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if app_context::track_game(&mut current_game, envelope.key, &envelope.payload) {
                continue;
            }
            match envelope.key {
                EventKey::ThinkingStart => {
                    let Some(text) = envelope.payload.get("userContent").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    if text.trim().is_empty() {
                        continue;
                    }
                    // Síncrono, inline — nunca o `.await` da extração (ver o cabeçalho do arquivo).
                    match prepare_extraction(&app, &mut gate, text, current_game.as_deref(), now_ms()) {
                        Ok(Some(job)) => {
                            tauri::async_runtime::spawn(run_extraction_job(job));
                        }
                        Ok(None) => {}
                        Err(error) => log::warn!("pipeline automático do person_catalog: pré-checagem falhou, seguindo sem catalogar: {error}"),
                    }
                }
                EventKey::ToolResult => {
                    handle_tool_result(&mut gate, &envelope.payload);
                }
                EventKey::PersonUpdated => {
                    if let Some(id) = created_person_id(&envelope.payload) {
                        let (app, id) = (app.clone(), id.to_string());
                        tauri::async_runtime::spawn(async move { enrich_created(&app, &id).await });
                    }
                }
                _ => {}
            }
        }
    });
}

/// Porte do listener `TOOL_RESULT` de `person-catalog-lifecycle.ts` (linha 71-76: chama
/// `notePersonToolUsed` quando `shouldNotePersonTool` aprova). Síncrono e infalível de propósito —
/// só atualiza o cooldown em memória, nunca toca disco nem rede.
fn handle_tool_result(gate: &mut MentionGate, payload: &serde_json::Value) {
    let Some(name) = payload.get("name").and_then(|v| v.as_str()) else {
        return;
    };
    let success = payload.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if should_note_person_tool_result(name, success) {
        gate.note_person_tool_used(now_ms());
    }
}

/// Everything [`run_extraction_job`] needs, resolved synchronously inside the subscriber loop.
struct ExtractionJob {
    app: AppHandle,
    config_store: Arc<ConfigStore>,
    catalog: &'static CatalogStore,
    input: ExtractionInput,
    max_mentions: usize,
}

/// Worth an LLM call? A capitalized word (a name), a relation ("minha esposa", "my brother") or an
/// open clarification the message may be answering ("o Pedro" is capitalized; "o mais velho" is
/// not, and is still an answer).
fn has_people_signal(text: &str, has_pending: bool) -> bool {
    has_pending || has_capitalized_word(text) || !super::people_memory::relations::relations_in_text(text).is_empty()
}

/// SYNCHRONOUS on purpose — the gate (grace period, minimum interval, repeated text) and its
/// bookkeeping run inline so ordering/dedup hold; only the LLM call leaves, to [`run_extraction_job`].
/// A relation or an open clarification skips the minimum interval: "minha esposa se chama Dani"
/// ten seconds after "minha esposa gosta de sorvete" must not be dropped.
fn prepare_extraction(app: &AppHandle, gate: &mut MentionGate, text: &str, game: Option<&str>, now: i64) -> Result<Option<ExtractionJob>, String> {
    let config_store = crate::state::config_store(app)?;
    if !auto_extract_enabled(&config_store) {
        return Ok(None);
    }
    let catalog = store_singleton::catalog_store(app)?;
    let pending = catalog.pending_clarifications();
    if !has_people_signal(text, !pending.is_empty()) {
        return Ok(None);
    }
    let urgent = !pending.is_empty() || !super::people_memory::relations::relations_in_text(text).is_empty();
    if !gate.should_extract_mention(text, now, urgent) {
        return Ok(None);
    }
    gate.begin_mention_extraction(now);
    gate.commit_mention_text(text, now);

    let character_name = config_store.get("character.name").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "Mika".to_string());
    let people = catalog.list(None, Some("lastSeenAt"), KNOWN_PEOPLE_HINT_LIMIT);
    let input = ExtractionInput {
        text: text.to_string(),
        known_people: mention_extract::known_people_lines(&people),
        pending: mention_extract::pending_lines(&pending, &catalog.list(None, None, usize::MAX)),
        app_context: game.map(app_context::extraction_context),
        character_name,
    };
    Ok(Some(ExtractionJob { app: app.clone(), max_mentions: max_mentions(&config_store), config_store, catalog, input }))
}

/// ASYNCHRONOUS — the network half, spawned, never awaited inline. Failures only warn.
async fn run_extraction_job(job: ExtractionJob) {
    let extracted = match mention_extract::extract(&job.config_store, &job.input).await {
        Ok(result) => result,
        Err(error) => {
            log::warn!("people memory: extraction failed, nothing catalogued: {error}");
            return;
        }
    };
    // The toggle may have been switched off while the model was answering.
    if !auto_extract_enabled(&job.config_store) {
        return;
    }
    let social_mem = crate::modules::memory::memory_store(&job.app).ok();
    let recording_allowed = crate::modules::memory::memory_recording_allowed(&job.config_store);
    ingest_extraction(job.catalog, social_mem, recording_allowed, &extracted, &job.input.text, now_ms(), job.max_mentions, &job.input.character_name);
}

/// `PersonUpdated{created:true}` from any path (chat, tool, watcher, settings RPC) — TS enriched on
/// `PERSON_CREATED`, so a person added from the settings UI also got the Wikipedia lookup.
fn created_person_id(payload: &serde_json::Value) -> Option<&str> {
    if payload.get("created").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    payload.get("personId").and_then(serde_json::Value::as_str)
}

async fn enrich_created(app: &AppHandle, id: &str) {
    let (Ok(config_store), Ok(catalog)) = (crate::state::config_store(app), store_singleton::catalog_store(app)) else {
        return;
    };
    if !web_enrichment_enabled(&config_store) {
        return;
    }
    let Some(person) = catalog.get(id) else { return };
    // People the user relates to (wife, brother, boss) are private: never looked up on the web.
    if person.enriched == Some(true) || person.relation_to_user.is_some() || !enrichment::is_enrichable(person.kind) {
        return;
    }
    let patch = enrichment::enrich_person(&config_store, &person).await;
    match catalog.apply_enrichment(&person.id, patch, now_ms()) {
        // The open detail refreshes with the bio/photo; `created: false` never re-enriches.
        Ok(Some(enriched)) => super::events::publish_updated(&enriched, false),
        Ok(None) => {}
        Err(error) => log::warn!("gravação do enriquecimento automático falhou: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_created_person_is_enriched() {
        assert_eq!(created_person_id(&serde_json::json!({ "personId": "ada", "created": true })), Some("ada"));
        assert_eq!(created_person_id(&serde_json::json!({ "personId": "ada", "created": false })), None);
        assert_eq!(created_person_id(&serde_json::json!({ "created": true })), None);
    }

    #[test]
    fn detects_a_capitalized_word() {
        assert!(has_capitalized_word("fala com a Ada depois"));
        assert!(!has_capitalized_word("oi tudo bem com voce"));
        assert!(!has_capitalized_word(""));
        // Unicode maiúscula sem par minúsculo em seguida (ex.: sigla) não conta — mesmo
        // comportamento de `/[A-Z][a-z]+/` no TS, que exige a letra minúscula depois.
        assert!(!has_capitalized_word("USB e HDMI"));
    }

    /// Regression: "minha esposa gosta de sorvete" typed in lowercase has no capitalized word and
    /// was never extracted; an answer like "o mais velho" to an open question neither.
    #[test]
    fn a_lowercase_relation_message_or_an_open_question_is_worth_extracting() {
        assert!(has_people_signal("minha esposa gosta de sorvete", false));
        assert!(has_people_signal("my wife likes ice cream", false));
        assert!(has_people_signal("o mais velho", true));
        assert!(!has_people_signal("vamo jogar mais uma", false));
    }

    #[test]
    fn tool_result_from_a_successful_person_catalog_call_starts_the_grace_period() {
        let mut gate = MentionGate::new();
        handle_tool_result(&mut gate, &serde_json::json!({ "name": "person_catalog", "success": true }));
        // Dentro da graça de 30s pós-escrita, uma extração na mesma janela é recusada.
        assert!(!gate.should_extract_mention("fala com a Ada", now_ms(), false), "devia estar em graça logo após o ToolResult");
    }

    #[test]
    fn tool_result_from_another_tool_or_a_failure_is_ignored() {
        let mut gate = MentionGate::new();
        handle_tool_result(&mut gate, &serde_json::json!({ "name": "web", "success": true }));
        assert!(gate.should_extract_mention("fala com a Ada", now_ms(), false), "tool errada não devia armar a graça");

        let mut gate = MentionGate::new();
        handle_tool_result(&mut gate, &serde_json::json!({ "name": "person_catalog", "success": false }));
        assert!(gate.should_extract_mention("fala com a Ada", now_ms(), false), "falha não devia armar a graça");
    }

    #[test]
    fn tool_result_missing_the_name_field_does_not_panic_or_arm_the_gate() {
        let mut gate = MentionGate::new();
        handle_tool_result(&mut gate, &serde_json::json!({ "success": true }));
        assert!(gate.should_extract_mention("fala com a Ada", now_ms(), false));
    }

    #[test]
    fn auto_extract_enabled_defaults_to_true_when_config_is_absent() {
        let (store, _dir) = test_config_store(None);
        assert!(auto_extract_enabled(&store));
    }

    #[test]
    fn auto_extract_enabled_only_false_when_explicitly_disabled() {
        let (store, _dir) = test_config_store(Some(false));
        assert!(!auto_extract_enabled(&store));
        let (store, _dir) = test_config_store(Some(true));
        assert!(auto_extract_enabled(&store));
    }

    #[test]
    fn auto_extract_enabled_is_false_when_master_toggle_is_off_even_with_auto_extract_true() {
        let (store, _dir) = test_config_store(Some(true));
        store.set("person-catalog.enabled", serde_json::json!(false)).unwrap();
        assert!(!auto_extract_enabled(&store), "person-catalog.enabled=false deve desligar a extração mesmo com autoExtractOnChat=true");
    }

    #[test]
    fn auto_extract_is_off_when_memory_recording_is_off() {
        let (store, _dir) = test_config_store(Some(true));
        store.set("memory.enabled", serde_json::json!(false)).unwrap();
        assert!(!auto_extract_enabled(&store), "memory.enabled=false não pode deixar o catálogo de pessoas gravando");
    }

    /// O nome de quem entra no catálogo só vai à Wikipédia/buscadores com o consentimento próprio
    /// (`webEnrichment`), que perfil novo não tem — `browser.enabled` ligado não o implica.
    #[test]
    fn web_enrichment_needs_its_own_opt_in() {
        let dir = TempDir::new();
        let store = ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap();
        assert!(!web_enrichment_enabled(&store), "perfil novo não consulta a web");

        store.set("browser.enabled", serde_json::json!(true)).unwrap();
        assert!(!web_enrichment_enabled(&store), "busca web ligada não autoriza o enriquecimento");

        store.set("person-catalog.webEnrichment", serde_json::json!(true)).unwrap();
        assert!(web_enrichment_enabled(&store));

        store.set("person-catalog.enabled", serde_json::json!(false)).unwrap();
        assert!(!web_enrichment_enabled(&store), "catálogo desligado não enriquece");
    }

    fn test_config_store(auto_extract: Option<bool>) -> (ConfigStore, TempDir) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "person-catalog", fields: vec![boolean("enabled", true), boolean("autoExtractOnChat", true)] },
            Namespace { name: "memory", fields: vec![boolean("enabled", true)] },
        ]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        if let Some(value) = auto_extract {
            store.set("person-catalog.autoExtractOnChat", serde_json::json!(value)).unwrap();
        }
        (store, dir)
    }

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            // Contador, não relógio: no Windows testes paralelos liam o mesmo nanossegundo.
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-person-catalog-lifecycle-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
