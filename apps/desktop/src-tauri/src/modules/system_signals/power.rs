//! Poller de energia: publica `EventKey::PowerResume`/`PowerBattery`/`PowerAc` no `EventBus` por transição —
//! port do recorte de `bootstrap-system-events.ts` que `awareness-event-handlers.ts` ainda reage
//! (`Events.POWER_RESUME`/`POWER_BATTERY`, ver o cabeçalho de `crate::power` para a lacuna que isto
//! revive em parte).
//!
//! **Sem evento nativo de suspend/resume.** O Electron tinha `powerMonitor.on('suspend'/'resume')`
//! do SO; não há crate equivalente já em `Cargo.toml` (`starship-battery`/`user-idle` só leem
//! ESTADO pontual, não empurram transição), e adicionar uma cross-plataforma nova está fora do
//! escopo cirúrgico desta rodada. Em vez disso, detecta resume pelo SALTO no relógio de parede
//! entre dois ticks — mesma técnica que o cabeçalho de `crate::power` já documentava como a forma
//! como o poller antigo fazia isso: enquanto o SO dorme, nem a CPU roda nem o timer do tokio avança,
//! mas `SystemTime::now()` (relógio de parede real) volta tendo pulado bem mais que o intervalo
//! esperado.
//!
//! Bateria é borda de estado (`false` -> `true` de [`crate::power::on_battery_power`]), não nível —
//! mesmo contrato do `powerMonitor.on('on-battery')`, que dispara só na DESCONEXÃO, não a cada
//! leitura.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::events::{EventBus, EventKey};

const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Salto de relógio de parede maior que isto entre dois ticks consecutivos conta como "o sistema
/// esteve suspenso nesse meio tempo". Folga generosa sobre `POLL_INTERVAL` para não confundir
/// jitter de agendamento do SO (segundos, não minutos) com uma suspensão real.
const RESUME_JUMP_THRESHOLD: Duration = Duration::from_secs(45);

fn at_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Puro: o salto entre dois ticks é grande demais para ser só jitter de agendamento.
fn is_resume_jump(elapsed_since_last_tick: Duration) -> bool {
    elapsed_since_last_tick > POLL_INTERVAL + RESUME_JUMP_THRESHOLD
}

/// Puro: só a borda `false` -> `true` conta como "acabou de desconectar da tomada" — mesmo
/// contrato de `powerMonitor.on('on-battery')`, que não dispara de novo enquanto já estava na
/// bateria.
fn is_battery_transition(was_on_battery: bool, is_on_battery_now: bool) -> bool {
    is_on_battery_now && !was_on_battery
}

/// Puro: a borda simétrica — `true` -> `false` é "acabou de religar na tomada". Mesmo poller, MESMA
/// leitura (`current_battery`), só a direção da borda muda — nenhuma leitura nova de hardware.
fn is_ac_transition(was_on_battery: bool, is_on_battery_now: bool) -> bool {
    !is_on_battery_now && was_on_battery
}

/// Sobe o laço. `tauri::async_runtime::spawn`, nunca `tokio::spawn` — mesma regra do resto do
/// boot (`modules/mod.rs::register_config_bound_tools` chama isto fora do reator). A leitura de
/// bateria é bloqueante (enumeração via `starship_battery::Manager`), por isso roda dentro de
/// `spawn_blocking` a cada tick em vez de direto no laço async.
pub fn spawn_power_signal_listener(bus: &'static EventBus) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        // O primeiro tick de um `tokio::time::interval` é IMEDIATO — descartado aqui para o
        // relógio de referência do próximo tick não nascer 0s atrás e acusar um "resume" falso
        // logo no boot.
        interval.tick().await;
        let mut last_tick = SystemTime::now();
        let mut on_battery = read_on_battery().await.unwrap_or(false);
        feed_governor(on_battery).await;
        let mut shutdown_rx = crate::shutdown::subscribe();

        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = interval.tick() => {}
            }
            let now = SystemTime::now();
            let elapsed = now.duration_since(last_tick).unwrap_or(Duration::ZERO);
            last_tick = now;

            if is_resume_jump(elapsed) {
                log::info!("poller de energia: retomada de suspensão detectada (salto de {elapsed:?})");
                bus.publish(EventKey::PowerResume, serde_json::json!({ "at": at_ms(now) }));
            }

            let current_battery = match read_on_battery().await {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("poller de energia: leitura de bateria falhou: {error}");
                    on_battery
                }
            };
            if is_battery_transition(on_battery, current_battery) {
                bus.publish(EventKey::PowerBattery, serde_json::json!({ "at": at_ms(now) }));
            }
            if is_ac_transition(on_battery, current_battery) {
                bus.publish(EventKey::PowerAc, serde_json::json!({ "at": at_ms(now) }));
            }
            on_battery = current_battery;
            feed_governor(on_battery).await;
        }
    });
}

/// Bateria e ócio do SO são os sinais de máquina do governador de energia — este poller já lê a
/// bateria a cada tick, e o ócio sai da mesma sonda barata (`GetLastInputInfo`/IOKit/XScreenSaver).
async fn feed_governor(on_battery: bool) {
    let idle = tokio::task::spawn_blocking(crate::power::idle_seconds).await.ok().and_then(Result::ok);
    crate::modules::system::governor::set_power_readings(on_battery, idle.map(Duration::from_secs));
}

/// A chamada síncrona (`starship_battery::Manager::batteries` enumera dispositivos via SO) roda
/// dentro de `spawn_blocking` para não travar o worker que também resolve `mika_invoke` — mesma
/// garantia de `inference/vad.rs`/`inference/whisper.rs`.
async fn read_on_battery() -> Result<bool, String> {
    tokio::task::spawn_blocking(crate::power::on_battery_power)
        .await
        .map_err(|error| format!("task de leitura de bateria falhou: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jump_within_the_expected_poll_cadence_is_not_a_resume() {
        assert!(!is_resume_jump(POLL_INTERVAL));
        assert!(!is_resume_jump(POLL_INTERVAL + Duration::from_secs(10)));
    }

    #[test]
    fn a_jump_well_past_the_poll_cadence_is_a_resume() {
        assert!(is_resume_jump(POLL_INTERVAL + RESUME_JUMP_THRESHOLD + Duration::from_secs(1)));
        assert!(is_resume_jump(Duration::from_secs(3600)));
    }

    #[test]
    fn plugging_in_is_not_a_battery_transition() {
        assert!(!is_battery_transition(true, false));
    }

    #[test]
    fn staying_on_battery_does_not_retrigger() {
        assert!(!is_battery_transition(true, true));
    }

    #[test]
    fn staying_plugged_in_does_not_trigger() {
        assert!(!is_battery_transition(false, false));
    }

    #[test]
    fn unplugging_is_the_only_transition_that_fires() {
        assert!(is_battery_transition(false, true));
    }

    #[test]
    fn unplugging_is_not_an_ac_transition() {
        assert!(!is_ac_transition(false, true));
    }

    #[test]
    fn staying_on_battery_does_not_retrigger_ac() {
        assert!(!is_ac_transition(true, true));
    }

    #[test]
    fn staying_plugged_in_does_not_trigger_ac() {
        assert!(!is_ac_transition(false, false));
    }

    #[test]
    fn plugging_in_is_the_only_ac_transition_that_fires() {
        assert!(is_ac_transition(true, false));
    }
}
