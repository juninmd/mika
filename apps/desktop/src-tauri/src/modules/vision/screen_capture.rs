//! Captura de tela GATEADA, com pixels — compartilhada por toda action que precisa mandar uma
//! imagem real para o LLM (`screenshot`/`watch_now` de `vision_capture`, `ocr` de `vision_read`) E
//! pelo observador periódico (`watcher.rs::run_tick`). Extraído de [`super::capture_tool`] nesta
//! rodada: antes só `frame` gateava (e descartava os pixels, só confirmava contagem de monitores);
//! agora QUALQUER chamador que precise dos bytes passa por aqui, nunca reimplementa o gate no
//! próprio handler — o mesmo chokepoint de privacidade deste repositório (`vision.enabled` →
//! `privacy::is_screen_capture_allowed` → blocklist de janela em FOCO → blocklist de janela
//! VISÍVEL), na ORDEM que os testes de `capture_tool` já fixam. Desde 2026-09-22 o chokepoint inclui
//! a checagem de foco (`capture_gate::is_foreground_blocked`, ver o cabeçalho de `capture_gate.rs`
//! para as diferenças de contrato com o TS) — antes só a varredura de janela VISÍVEL existia.

use serde_json::{json, Value};

use super::capture_gate::{self, is_blocked_window_title, parse_blocklist};
use super::responses::{capture_unavailable_result, disabled_result};
use crate::config::store::ConfigStore;
use crate::modules::privacy;

pub(super) fn is_vision_enabled(store: &ConfigStore) -> bool {
    store.get("vision.enabled").and_then(|value| value.as_bool()).unwrap_or(false)
}

/// Metade PURA de `isCaptureBlocked` que este backend consegue honrar — ver o cabeçalho de
/// `capture_gate.rs`. Falha de leitura (config ausente, `capture::get_sources` retornando `Err`)
/// NÃO bloqueia: o gate OBRIGATÓRIO é `is_screen_capture_allowed`, chamado antes desta função; esta
/// varredura é a defesa SECUNDÁRIA, best-effort.
pub(super) fn any_visible_window_blocked(store: &ConfigStore) -> bool {
    let raw = store.get("vision.captureBlocklist");
    let patterns = parse_blocklist(raw.as_ref().and_then(Value::as_str));
    if patterns.is_empty() {
        return false;
    }
    let Ok(Value::Array(sources)) = crate::capture::get_sources(&json!({ "types": ["window"] })) else {
        return false;
    };
    sources.iter().any(|source| is_blocked_window_title(source["name"].as_str(), &patterns).is_some())
}

/// Frames crus de TODOS os monitores, já gateados — sem JPEG intermediário: quem manda ao LLM
/// codifica uma vez só (`llm_client::prepare_frames`). `Err` carrega o mesmo corpo de resposta
/// `{ success: false, ... }` que os handlers devolvem direto ao chamador — nunca um erro genérico
/// que esconderia qual dos três motivos (plugin desligado, consentimento negado, janela bloqueada)
/// recusou a captura.
pub(super) fn capture_screen_frames(store: &ConfigStore) -> Result<Vec<image::DynamicImage>, Value> {
    if !is_vision_enabled(store) {
        return Err(disabled_result());
    }
    // O CHOKEPOINT: checado aqui, no adaptador, antes de qualquer captura — nunca pelo chamador.
    if !privacy::is_screen_capture_allowed(store) {
        return Err(capture_unavailable_result());
    }
    // Mesma ORDEM do TS (`isCaptureBlocked`: foco primeiro, depois qualquer janela visível).
    if capture_gate::is_foreground_blocked(store) {
        return Err(capture_unavailable_result());
    }
    if any_visible_window_blocked(store) {
        return Err(capture_unavailable_result());
    }
    match crate::capture::capture_monitor_frames() {
        Ok(frames) if !frames.is_empty() => Ok(frames),
        _ => Err(capture_unavailable_result()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-vision-screen-capture-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store_with(enabled: bool, screen_capture: bool) -> (ConfigStore, TempDir) {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::{boolean, text};

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "vision", fields: vec![boolean("enabled", false), text("captureBlocklist", "")] },
            Namespace { name: "privacy", fields: vec![boolean("screenCapture", false)] },
        ]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        store.set("vision.enabled", json!(enabled)).unwrap();
        store.set("privacy.screenCapture", json!(screen_capture)).unwrap();
        (store, dir)
    }

    #[test]
    fn refuses_with_the_disabled_result_when_the_vision_toggle_is_off() {
        let (store, _dir) = store_with(false, true);
        assert_eq!(capture_screen_frames(&store).err(), Some(disabled_result()));
    }

    /// NEGATIVO — `vision.enabled=true` não basta, `privacy.screenCapture` continua sendo quem
    /// decide: o mesmo chokepoint que `capture_tool::frame` já prova, agora aqui também.
    #[test]
    fn refuses_with_the_capture_unavailable_result_when_privacy_screen_capture_is_off() {
        let (store, _dir) = store_with(true, false);
        assert_eq!(capture_screen_frames(&store).err(), Some(capture_unavailable_result()));
    }

    #[test]
    fn the_disabled_check_runs_before_the_privacy_check() {
        let (store, _dir) = store_with(false, false);
        assert_eq!(capture_screen_frames(&store).err(), Some(disabled_result()));
    }
}
