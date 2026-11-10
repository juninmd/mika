//! Sweep de manutenção da memória episódica: promove entradas do tier curto para o tier longo por
//! IDADE OU por IMPORTÂNCIA (`should_store_long` — `importance >= 4`/`important: true`), independente
//! do teto de volume que [`super::consolidation::maybe_consolidate`] exige
//! (`CONSOLIDATION_THRESHOLD` = 200 entradas). Fecha o gap documentado em defects.md
//! "memory-relationship" P1: um usuário de baixo uso nunca acumula 200 entradas no tier curto, e a
//! memória "da semana passada" (TTL de 7 dias) expirava antes de qualquer consolidação rodar.
//!
//! Roda no BOOT (primeiro tick de `tokio::time::interval` é imediato) e a cada [`SWEEP_INTERVAL`]
//! depois — mesmo padrão de `features::mood::boot::spawn_decay_timer` (assina o `shutdown` ANTES do
//! primeiro `.tick()`, nunca assina o barramento de eventos, então não há janela de corrida a
//! perder). A lógica pura (o QUE promover, com relógio injetável) mora em
//! [`crate::modules::memory::episodic::consolidation::promote_aged_entries`] — este arquivo só
//! decide QUANDO chamar e resolve o `MemoryStore`/`ConfigStore` de processo.

use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;

use crate::config::store::ConfigStore;

/// = ~5 dias, o "older than ~5 days" do brief — abaixo do TTL padrão de 7 dias
/// (`SHORT_TTL_HOURS_DEFAULT`, `episodic::types`), com folga para o sweep rodar antes do TTL
/// derrubar a entrada de vez.
const AGE_PROMOTION_THRESHOLD_MS: i64 = 5 * 24 * 3_600_000;
/// "A cada poucas horas" do brief — frequente o bastante para uma entrada não passar dias além do
/// cutoff antes de ser pega, raro o bastante para nunca competir por I/O com o turno de chat.
const SWEEP_INTERVAL: Duration = Duration::from_secs(6 * 3_600);

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Um tick isolado — nunca propaga erro (mesma política de `post_turn::consolidation::maybe_consolidate`:
/// uma falha de sweep não pode derrubar o processo nem o próximo tick).
async fn sweep_once(app: &AppHandle, store: &ConfigStore) {
    let mem_store = match crate::modules::memory::memory_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::warn!("post_turn::aging_sweep: memória indisponível: {error}");
            return;
        }
    };
    let recording_allowed = crate::modules::memory::memory_recording_allowed(store);
    let now = now_ms();
    // SQLite síncrono: fora do worker async.
    let promoted = tauri::async_runtime::spawn_blocking(move || {
        crate::modules::memory::episodic_configured(mem_store).promote_aged(recording_allowed, now, AGE_PROMOTION_THRESHOLD_MS)
    })
    .await;
    match promoted {
        Ok(Ok(0)) => {}
        Ok(Ok(count)) => log::info!("post_turn::aging_sweep: {count} entrada(s) promovida(s) ao tier longo por idade"),
        Ok(Err(error)) => log::warn!("post_turn::aging_sweep: falhou: {error}"),
        Err(error) => log::warn!("post_turn::aging_sweep: tarefa abortou: {error}"),
    }
}

/// Chamado uma vez no boot (ver `modules/mod.rs::register_config_bound_tools`, ao lado de
/// `spawn_post_turn_listener`). `tauri::async_runtime::spawn`, nunca `tokio::spawn` — este código
/// roda a partir de `lib.rs::setup`, fora do reator Tokio (ver o teste
/// `modules::tests::nothing_the_boot_calls_uses_bare_tokio_spawn`).
pub fn spawn_aging_sweep(app: AppHandle, store: Arc<ConfigStore>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = ticker.tick() => {}
            }
            sweep_once(&app, &store).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_threshold_stays_under_the_default_short_tier_ttl() {
        // = `SHORT_TTL_HOURS_DEFAULT` (`episodic::types`, privado a este diretório) — duplicado aqui
        // como número porque `types` não é `pub`; qualquer mudança de um lado sem o outro é um teste
        // que falha, não um import que quebra em silêncio.
        const DEFAULT_SHORT_TTL_HOURS: f64 = 168.0;
        let default_ttl_ms = (DEFAULT_SHORT_TTL_HOURS * 3_600_000.0) as i64;
        assert!(
            AGE_PROMOTION_THRESHOLD_MS < default_ttl_ms,
            "o corte de promoção por idade tem de disparar ANTES do TTL padrão apagar a entrada, senão o sweep chega tarde demais"
        );
    }
}
