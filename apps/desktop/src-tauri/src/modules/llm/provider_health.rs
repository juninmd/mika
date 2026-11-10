//! Estado REAL de saúde de provedor — registro em memória de processo, atualizado pelos turnos de
//! chat de verdade (`chat::send::handle_chat_send`), lido por `llm::get_provider_health`
//! (`llm:getProviderHealth`) e empurrado ao renderer via `llm:provider-health`
//! (`LLMProviderHealth`, `apps/desktop/src/shared/electron-api-system-types.ts`).
//!
//! Antes desta rodada, `get_provider_health` devolvia sempre `{ health: [] }` — honesto enquanto o
//! chat rodava no sidecar Node (era ELE quem observava falha real de provedor), mas esse comentário
//! ficou desatualizado: `chat::send::handle_chat_send` já roda o turno inteiro em Rust
//! (`brain::turn::run_turn_llm`) desde a onda `6b9bb6ec`. Devolver `[]` sempre não era mais uma
//! resposta honesta sobre "não implementei" — era uma resposta ERRADA sobre "está tudo bem", porque
//! nada em Rust nunca marcava nada como falho. `ProviderHealthBanner`/`useProviderHealthIndicator`
//! (renderer) dependem deste canal para avisar o usuário que o provedor está fora — com o registro
//! sempre vazio, uma falha real de provedor nunca aparecia na tela, só o erro avulso daquele turno.
//!
//! Limiar de 3 falhas seguidas antes de `disabled`: mesmo número e mesmo motivo de
//! `turn::failure_tracker::MAX_FAILURES_BEFORE_BLOCK` (tools) — convenção já aceita neste repositório
//! para "quantas vezes seguidas antes de tratar como não-confiável", não um valor novo inventado
//! aqui. Entre 1 e 2 falhas o estado é `degraded` (visível em `getProviderHealth`, mas
//! `ProviderHealthBanner`/`useProviderHealthIndicator` só reagem a `disabled` — a mesma UI que já
//! existia, sem mudar o que ela decide mostrar).
//!
//! Sem retry/backoff automático (o `llm-circuit-state.ts` do TS tinha timer de retry programado) —
//! fora do escopo desta rodada; `resetProviderCircuit` (manual, botão "Tentar de novo" do banner) já
//! existe e agora tem o que resetar de verdade.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// Mesmo limiar e mesmo motivo de `turn::failure_tracker::MAX_FAILURES_BEFORE_BLOCK` — ver o
/// cabeçalho do arquivo.
const MAX_FAILURES_BEFORE_DISABLED: u32 = 3;

/// Cooldown de um provedor `disabled` por falha TRANSIENTE (429/5xx/rede) antes de deixar passar
/// UMA chamada de sondagem (`is_blocked` volta a liberar) — mesmo minuto citado em 03-resilience.md
/// ("half-open só religaria o loop a cada minuto"), não um valor novo inventado aqui. Falha FATAL
/// (`HealthEntry::fatal`) nunca usa este cooldown — ver [`is_blocked`].
const TRANSIENT_COOLDOWN_MS: u64 = 60_000;

#[derive(Debug, Clone, PartialEq)]
pub struct HealthEntry {
    provider: String,
    model: String,
    state: &'static str,
    reason: Option<String>,
    status_code: Option<u16>,
    since_ms: u64,
    failure_count: u32,
    /// `true` quando alguma falha registrada para este par já foi classificada como FATAL
    /// (`is_fatal_status`) — sticky: uma vez fatal, o par não volta a ser "só transiente" sem um
    /// sucesso ou reset explícito. Decide a política de [`is_blocked`] (sem cooldown automático).
    fatal: bool,
    /// Superfícies (`"chat"`, `"vision"`, …) que já reportaram falha para este `provider:model`
    /// (WP9 item 6) — some para o `ProviderHealthBanner` distinguir "Mika parou de ver a tela"
    /// (só `vision`) de uma falha de chat, quando o mesmo par é usado nas duas superfícies. STICKY
    /// e ADITIVO como `fatal`: uma segunda superfície falhando no mesmo par é uma falha REAL a
    /// mais, nunca substitui a primeira.
    surfaces: Vec<&'static str>,
}

impl HealthEntry {
    /// `pub`: `chat::send::handle_chat_send` precisa do payload exato para emitir
    /// `llm:provider-health` ao renderer depois de [`record_failure`]/[`reset`].
    pub fn to_json(&self) -> Value {
        json!({
            "key": key(&self.provider, &self.model),
            "provider": self.provider,
            "model": self.model,
            "state": self.state,
            "reason": self.reason,
            "statusCode": self.status_code,
            "since": self.since_ms,
            "failureCount": self.failure_count,
            "fatal": self.fatal,
            "surfaces": self.surfaces,
        })
    }
}

/// **MUST** (03-resilience.md, "Duas classes de falha"): 4xx é fatal, EXCETO 429 (rate limit,
/// transiente por natureza — tabela da spec) e ausência de status (rede/timeout, sempre
/// transiente). Nunca usada para decidir se HOUVE falha, só a POLÍTICA depois que houve.
pub fn is_fatal_status(status_code: Option<u16>) -> bool {
    matches!(status_code, Some(code) if (400..500).contains(&code) && code != 429)
}

/// O 404 do OpenRouter "No endpoints found matching your data policy": o modelo escolhido não tem
/// endpoint que cumpra o filtro ZDR do usuário. É a política funcionando, não o provedor quebrado —
/// não conta strike (três mensagens "desativariam" o par e o banner esconderia a causa real: trocar
/// de modelo).
pub fn is_data_policy_mismatch(reason: &str) -> bool {
    reason.to_lowercase().contains("matching your data policy")
}

/// O status HTTP embutido no texto de um erro do `rig-core`. Duas grafias, porque o `Display` mudou na
/// 0.43: `HttpError: Invalid status code 404 Not Found with message: …` (≤ 0.42) e
/// `ProviderResponseError: status 404 Not Found: …` (≥ 0.43, `ProviderError::ProviderResponse`, de
/// onde vêm todas as respostas HTTP de erro de um provedor).
///
/// O status vem do TEXTO porque o erro chega até aqui como `String`. Pinado ao texto exato da versão
/// (`rig-core-0.45.0/src/provider_response.rs`): se a dependência mudar e o texto mudar, isto só
/// para de casar — a falha some, cai em `None`, e a política já trata `None` como transiente (nunca
/// panica, nunca finge fatal por engano); o teste logo abaixo trava o formato.
pub fn extract_http_status(err: &str) -> Option<u16> {
    ["Invalid status code ", "ProviderResponseError: status "].iter().find_map(|marker| {
        let after_marker = err.split(marker).nth(1)?;
        after_marker.split_whitespace().next()?.parse().ok()
    })
}

/// Payload EFÊMERO de "voltou a funcionar" — nunca gravado no registro (ver o comentário de
/// [`record_success`]: um provedor saudável não ocupa entrada), só emitido ao renderer para que
/// `ProviderHealthBanner`/`useProviderHealthIndicator` recarreguem e removam o aviso.
pub fn healthy_payload(provider: &str, model: &str) -> Value {
    json!({
        "key": key(provider, model),
        "provider": provider,
        "model": model,
        "state": "healthy",
        "reason": Value::Null,
        "statusCode": Value::Null,
        "since": now_ms(),
        "failureCount": 0,
        "fatal": false,
        "surfaces": Value::Array(Vec::new()),
    })
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn key(provider: &str, model: &str) -> String {
    format!("{provider}:{model}")
}

fn registry() -> &'static Mutex<HashMap<String, HealthEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, HealthEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Um turno bem-sucedido zera a contagem e volta o provedor a `healthy` — some do registro em vez
/// de ficar como uma entrada `healthy` permanente, porque o caso comum (tudo funcionando) não deveria
/// crescer o registro para sempre. Devolve a entrada REMOVIDA quando havia uma falha registrada antes
/// (para quem chama decidir se emite o evento de "voltou a funcionar" — silêncio quando não havia
/// nada a corrigir).
pub fn record_success(provider: &str, model: &str) -> Option<HealthEntry> {
    registry().lock().ok()?.remove(&key(provider, model))
}

/// Cada falha REAL de turno (erro do provedor, nunca abort do usuário — ver `handle_chat_send`)
/// incrementa a contagem. Uma falha FATAL ([`is_fatal_status`]) trava na PRIMEIRA vez (`disabled`
/// imediato, "Latch fatal" em 03-resilience.md); uma falha TRANSIENTE só desativa a partir de
/// [`MAX_FAILURES_BEFORE_DISABLED`] seguidas. Sempre devolve a entrada atualizada — quem chama
/// decide se emite.
///
/// Assinatura preservada (chamadores: `chat::send::handle_chat_send`, `brain::turn::failover_outcome`
/// — ambos superfície `"chat"`, o mesmo pipeline de turno de chat) para não alargar o raio de uma
/// mudança de WP9 a arquivos fora do escopo desta rodada; ver [`record_failure_for_surface`].
pub fn record_failure(provider: &str, model: &str, reason: &str, status_code: Option<u16>) -> HealthEntry {
    record_failure_for_surface(provider, model, reason, status_code, "chat")
}

/// Igual a [`record_failure`], mas nomeando a superfície que observou a falha (WP9 item 6 —
/// `vision::watcher` é o único chamador fora de `"chat"` nesta rodada, para que uma falha
/// só-de-visão apareça como tal no banner em vez de indistinguível de uma falha de chat).
pub fn record_failure_for_surface(provider: &str, model: &str, reason: &str, status_code: Option<u16>, surface: &'static str) -> HealthEntry {
    let mut guard = registry().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = guard.entry(key(provider, model)).or_insert_with(|| HealthEntry {
        provider: provider.to_string(),
        model: model.to_string(),
        state: "degraded",
        reason: None,
        status_code: None,
        since_ms: now_ms(),
        failure_count: 0,
        fatal: false,
        surfaces: Vec::new(),
    });
    entry.failure_count += 1;
    entry.reason = Some(reason.to_string());
    entry.status_code = status_code;
    entry.since_ms = now_ms();
    // Sticky: uma vez fatal, continua fatal mesmo que uma falha posterior (ex.: rede caiu enquanto
    // o modelo inválido também estava configurado) venha sem status classificável.
    entry.fatal = entry.fatal || is_fatal_status(status_code);
    entry.state = if entry.fatal || entry.failure_count >= MAX_FAILURES_BEFORE_DISABLED { "disabled" } else { "degraded" };
    if !entry.surfaces.contains(&surface) {
        entry.surfaces.push(surface);
    }
    entry.clone()
}

/// `llm:getProviderHealth`/emissão pontual: a entrada atual de UM par, sem exigir `disabled` (ao
/// contrário de [`is_blocked`]) — usado por `chat::send::handle_chat_send` para emitir
/// `llm:provider-health` do par CONFIGURADO logo depois que um failover grava a falha dele (ver o
/// comentário de `TurnResult::served_by_failover`), mesmo quando ainda só `degraded`.
pub fn get(provider: &str, model: &str) -> Option<HealthEntry> {
    registry().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(&key(provider, model)).cloned()
}

/// **MUST** (03-resilience.md, "Latch fatal"): uma chamada para um par `provider:model` travado
/// falha ANTES de qualquer I/O — quem chama consulta isto e nunca sai para a rede quando volta
/// `Some`. Falha FATAL trava para sempre (até `reset`/sucesso/mudança de config, que troca a
/// chave). Falha TRANSIENTE libera sozinha depois de [`TRANSIENT_COOLDOWN_MS`] (half-open: UMA
/// tentativa passa; se falhar de novo, [`record_failure`] atualiza `since_ms` e o cooldown recomeça).
pub fn is_blocked(provider: &str, model: &str) -> Option<HealthEntry> {
    let mut guard = registry().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    blocked_at(guard.get_mut(&key(provider, model))?, now_ms())
}

/// Decisão do latch sob o lock do registro. Quem passa pela meia-abertura REARMA `since_ms` na
/// mesma seção crítica: um segundo chamador concorrente (chat + watcher) volta a ver bloqueado, e
/// a meia-abertura deixa passar UMA tentativa, não todas as que chegarem na janela.
fn blocked_at(entry: &mut HealthEntry, now: u64) -> Option<HealthEntry> {
    if entry.state != "disabled" {
        return None;
    }
    if entry.fatal || now.saturating_sub(entry.since_ms) < TRANSIENT_COOLDOWN_MS {
        return Some(entry.clone());
    }
    entry.since_ms = now;
    None
}

/// `llm:resetProviderCircuit`: limpa a entrada manualmente (botão "Tentar de novo" do banner).
/// `None` quando não havia nada para resetar — resposta honesta, não um `ok: true` fingido (mesma
/// regra que este arquivo já seguia antes desta rodada).
pub fn reset(provider: &str, model: &str) -> Option<HealthEntry> {
    registry().lock().ok()?.remove(&key(provider, model))
}

/// `llm:getProviderHealth`: todas as entradas com falha real registrada (nunca inclui provedor
/// silenciosamente saudável — ver o comentário de [`record_success`]).
pub fn snapshot() -> Vec<Value> {
    let guard = registry().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.values().map(HealthEntry::to_json).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disabled_transient(since_ms: u64) -> HealthEntry {
        HealthEntry { provider: "p".into(), model: "m".into(), state: "disabled", reason: None, status_code: Some(503), since_ms, failure_count: 3, fatal: false, surfaces: Vec::new() }
    }

    /// Meia-abertura: passado o cooldown, UMA tentativa passa e rearma o latch — a segunda
    /// chamada na mesma janela volta a ver bloqueado em vez de também sair para a rede.
    #[test]
    fn the_half_open_window_lets_exactly_one_probe_through() {
        let mut entry = disabled_transient(0);
        let after = TRANSIENT_COOLDOWN_MS + 5;

        assert!(blocked_at(&mut entry, 10).is_some(), "ainda no cooldown");
        assert!(blocked_at(&mut entry, after).is_none(), "primeira tentativa passa");
        assert!(blocked_at(&mut entry, after + 1).is_some(), "segunda na mesma janela é barrada");
    }

    #[test]
    fn a_fatal_entry_never_opens_by_time() {
        let mut entry = HealthEntry { fatal: true, ..disabled_transient(0) };
        assert!(blocked_at(&mut entry, TRANSIENT_COOLDOWN_MS * 10).is_some());
    }

    /// Isola cada teste: o registro é `static`, de processo — sem um provider/model exclusivo por
    /// teste, o `cargo test` paralelo cruzaria escrita de um teste com leitura de outro.
    fn unique_key(tag: &str) -> (String, String) {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        (format!("test-provider-{tag}-{n}"), "test-model".to_string())
    }

    #[test]
    fn only_the_openrouter_data_policy_404_is_a_policy_mismatch() {
        assert!(is_data_policy_mismatch("Invalid status code 404 Not Found: No endpoints found matching your data policy (Zero data retention)"));
        assert!(!is_data_policy_mismatch("Invalid status code 404 Not Found: model not found"));
    }

    #[test]
    fn a_healthy_provider_never_appears_in_the_registry() {
        let (provider, model) = unique_key("healthy");
        assert!(record_success(&provider, &model).is_none());
        assert!(!snapshot().iter().any(|v| v["provider"] == provider));
    }

    #[test]
    fn one_or_two_failures_are_degraded_not_disabled() {
        let (provider, model) = unique_key("degraded");
        let first = record_failure(&provider, &model, "timeout", None);
        assert_eq!(first.state, "degraded");
        assert_eq!(first.failure_count, 1);

        let second = record_failure(&provider, &model, "timeout again", Some(500));
        assert_eq!(second.state, "degraded");
        assert_eq!(second.failure_count, 2);
        assert_eq!(second.status_code, Some(500));
    }

    /// O limiar central desta rodada: a TERCEIRA falha seguida do MESMO provider+model vira
    /// `disabled` — mesma contagem de `turn::failure_tracker` para tools.
    #[test]
    fn three_consecutive_failures_disable_the_provider() {
        let (provider, model) = unique_key("disabled");
        record_failure(&provider, &model, "erro 1", None);
        record_failure(&provider, &model, "erro 2", None);
        let third = record_failure(&provider, &model, "erro 3", Some(502));
        assert_eq!(third.state, "disabled");
        assert_eq!(third.failure_count, 3);

        let found = snapshot().into_iter().find(|v| v["provider"] == provider).expect("entrada deveria existir");
        assert_eq!(found["state"], "disabled");
        assert_eq!(found["key"], key(&provider, &model));
    }

    // -- is_fatal_status / extract_http_status ------------------------------------------------------

    #[test]
    fn is_fatal_status_classifies_4xx_as_fatal_except_429() {
        assert!(is_fatal_status(Some(400)));
        assert!(is_fatal_status(Some(404)));
        assert!(is_fatal_status(Some(401)));
        assert!(!is_fatal_status(Some(429)), "429 é rate limit, transiente por natureza (tabela da spec)");
    }

    #[test]
    fn is_fatal_status_classifies_5xx_network_and_absent_status_as_transient() {
        assert!(!is_fatal_status(Some(500)));
        assert!(!is_fatal_status(Some(503)));
        assert!(!is_fatal_status(None));
    }

    #[test]
    fn extract_http_status_reads_the_rig_core_http_error_display_format() {
        let err = "erro do provedor: HttpError: Invalid status code 404 Not Found with message: {\"error\":\"model not found\"}";
        assert_eq!(extract_http_status(err), Some(404));
    }

    /// Desde a 0.43 toda resposta HTTP de erro de um provedor é `ProviderError::ProviderResponse`, cujo
    /// `Display` não tem mais "Invalid status code" — sem esta grafia o 404 de modelo inexistente
    /// deixava de travar o par na primeira falha e o 401/403 de chave inválida deixava de ser fatal.
    #[test]
    fn extract_http_status_reads_the_provider_response_display_format() {
        let err = "erro do provedor: ProviderResponseError: status 404 Not Found: {\"error\":\"model not found\"}";
        assert_eq!(extract_http_status(err), Some(404));
        assert_eq!(extract_http_status("ProviderResponseError: status 429 Too Many Requests: slow down (request id: abc)"), Some(429));
    }

    #[test]
    fn extract_http_status_is_none_when_the_marker_is_absent() {
        assert_eq!(extract_http_status("timeout de conexão"), None);
    }

    // -- latch fatal ----------------------------------------------------------------------------------

    /// O coração desta rodada: um erro FATAL (modelo inválido, 404) trava na PRIMEIRA falha, não na
    /// terceira — "Latch fatal" em 03-resilience.md.
    #[test]
    fn a_single_fatal_failure_disables_the_provider_immediately() {
        let (provider, model) = unique_key("fatal-latch");
        let entry = record_failure(&provider, &model, "modelo inválido", Some(404));
        assert_eq!(entry.state, "disabled");
        assert_eq!(entry.failure_count, 1);
        assert!(entry.fatal);
    }

    /// **MUST**: uma chamada bloqueada pelo latch falha ANTES de qualquer I/O — `is_blocked` é o
    /// que o chamador consulta antes de sair para a rede.
    #[test]
    fn is_blocked_returns_the_entry_for_a_fatal_disabled_pair_and_none_for_a_healthy_one() {
        let (provider, model) = unique_key("fatal-blocked");
        assert!(is_blocked(&provider, &model).is_none(), "par saudável nunca deveria bloquear");

        record_failure(&provider, &model, "modelo inválido", Some(404));
        let blocked = is_blocked(&provider, &model).expect("falha fatal deveria travar antes de qualquer I/O");
        assert_eq!(blocked.state, "disabled");
    }

    /// Uma falha TRANSIENTE (429/5xx/rede) continua exigindo 3 seguidas antes de travar — só a
    /// FATAL trava na primeira.
    #[test]
    fn transient_failures_still_need_three_strikes_to_block() {
        let (provider, model) = unique_key("transient-strikes");
        record_failure(&provider, &model, "rate limit", Some(429));
        assert!(is_blocked(&provider, &model).is_none(), "1ª falha transiente não deveria bloquear ainda");
        record_failure(&provider, &model, "rate limit", Some(429));
        assert!(is_blocked(&provider, &model).is_none(), "2ª falha transiente não deveria bloquear ainda");
        record_failure(&provider, &model, "rate limit", Some(429));
        assert!(is_blocked(&provider, &model).is_some(), "3ª falha transiente trava (mesmo limiar de sempre)");
    }

    /// Falha fatal é STICKY: mesmo se uma falha posterior chegar sem status (rede caiu no meio),
    /// o par continua fatal — nunca "esquece" a falha permanente por causa de um erro transiente
    /// depois dela.
    #[test]
    fn a_fatal_entry_stays_fatal_after_a_later_status_less_failure() {
        let (provider, model) = unique_key("fatal-sticky");
        record_failure(&provider, &model, "modelo inválido", Some(404));
        let after = record_failure(&provider, &model, "timeout", None);
        assert!(after.fatal, "uma vez fatal, continua fatal");
        assert_eq!(after.state, "disabled");
    }

    /// Um turno bem-sucedido DEPOIS de falhas zera tudo — o provedor não fica `disabled` para
    /// sempre só porque um dia falhou 3 vezes.
    #[test]
    fn a_success_after_failures_clears_the_entry() {
        let (provider, model) = unique_key("recovers");
        record_failure(&provider, &model, "erro", None);
        record_failure(&provider, &model, "erro", None);
        record_failure(&provider, &model, "erro", None);
        assert!(snapshot().iter().any(|v| v["provider"] == provider));

        let cleared = record_success(&provider, &model);
        assert!(cleared.is_some(), "deveria devolver a entrada removida");
        assert!(!snapshot().iter().any(|v| v["provider"] == provider));
    }

    #[test]
    fn reset_clears_a_disabled_entry_and_is_honest_when_there_is_nothing_to_reset() {
        let (provider, model) = unique_key("reset");
        assert!(reset(&provider, &model).is_none(), "nada para resetar ainda deveria ser None, não fingir sucesso");

        record_failure(&provider, &model, "erro", None);
        let reset_entry = reset(&provider, &model);
        assert!(reset_entry.is_some());
        assert!(!snapshot().iter().any(|v| v["provider"] == provider));
    }

    /// O payload de recuperação NUNCA é gravado no registro — só existe para o evento.
    #[test]
    fn healthy_payload_is_ephemeral_never_stored() {
        let (provider, model) = unique_key("healthy-payload");
        let payload = healthy_payload(&provider, &model);
        assert_eq!(payload["state"], "healthy");
        assert_eq!(payload["failureCount"], 0);
        assert!(!snapshot().iter().any(|v| v["provider"] == provider), "healthy_payload não deveria gravar nada");
    }

    #[test]
    fn health_entry_json_matches_the_llm_provider_health_contract() {
        let (provider, model) = unique_key("json-shape");
        let entry = record_failure(&provider, &model, "erro de rede", Some(429));
        let json = entry.to_json();
        for field in ["key", "provider", "model", "state", "reason", "statusCode", "since", "failureCount", "surfaces"] {
            assert!(json.get(field).is_some(), "campo {field} ausente — quebraria LLMProviderHealth no renderer");
        }
    }

    // -- surfaces (WP9 item 6) -------------------------------------------------------------------

    /// Reprodução do gap: antes desta rodada, uma falha SÓ de visão era indistinguível de uma falha
    /// de chat no mesmo par `provider:model` — o banner não tinha como dizer "Mika parou de ver a
    /// tela" de "o chat está fora".
    #[test]
    fn record_failure_defaults_to_the_chat_surface() {
        let (provider, model) = unique_key("chat-surface-default");
        let entry = record_failure(&provider, &model, "erro", None);
        assert_eq!(entry.surfaces, vec!["chat"]);
    }

    #[test]
    fn record_failure_for_surface_tags_the_reporting_surface() {
        let (provider, model) = unique_key("vision-surface");
        let entry = record_failure_for_surface(&provider, &model, "erro do provedor de visão", None, "vision");
        assert_eq!(entry.surfaces, vec!["vision"]);
    }

    /// Duas superfícies reais falhando no MESMO par não se sobrescrevem — o banner precisa saber
    /// que as duas estão afetadas, não só a última que reportou.
    #[test]
    fn a_second_surface_failing_on_the_same_pair_is_additive_not_overwritten() {
        let (provider, model) = unique_key("shared-pair-surfaces");
        record_failure_for_surface(&provider, &model, "erro de visão", None, "vision");
        let after_chat = record_failure_for_surface(&provider, &model, "erro de chat", None, "chat");
        assert_eq!(after_chat.surfaces, vec!["vision", "chat"]);

        // Uma terceira falha da MESMA superfície não duplica a entrada.
        let repeated = record_failure_for_surface(&provider, &model, "erro de visão de novo", None, "vision");
        assert_eq!(repeated.surfaces, vec!["vision", "chat"]);
    }
}
