//! Tool `get_system_info` — porte de `createSystemInfoTool`
//! (`packages/sdk/src/tools/tool-registry.ts` + `packages/sdk/src/tools/system-info-tool.ts`).
//!
//! Único registro do catálogo TS que não pertence a nenhuma feature: `ToolRegistryService.init()`
//! chama `this.register({ name: 'get_system_info', ... }, 'core')` direto no construtor, fora de
//! `bootstrap/tools.ts::registerFeatureWithTools`. Config-livre — a tool não lê `ConfigStore`, só
//! informação do próprio processo — por isso registra como efeito colateral de
//! [`super::modules()`], sem precisar de uma segunda passada com `AppHandle`.
//!
//! ## `machine` apart from `mikaProcess`
//!
//! The TS port returned a bare `memory` (Mika's own RSS) and `uptime` (since the app opened). Asked
//! "como está meu PC?", the model read 875MB of Mika's RSS as "875MB livres" and the app's uptime
//! as the boot time. Machine-wide figures and Mika's own now live under separate, labelled keys.

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::modules::tools::{registry, ToolDefinition, ToolHandler};

/// Cópia literal de `createSystemInfoTool().description` (`system-info-tool.ts`).
const DESCRIPTION: &str = "Health of the machine Mika runs on: platform, CPU arch, `machine` (RAM used/total, CPU load, uptime), `mikaProcess` (your own memory and uptime). ONLY when asked about the computer itself — \"is it slow?\", \"how much memory?\" (pt-BR \"está pesado?\"). Knows nothing about the user, screen, files or apps: use desktop or vision.";

/// Espelha `process.platform` (Node) a partir de `std::env::consts::OS`. Só os três valores que a
/// Mika roda de verdade (`AGENTS.md` §1: Windows/Linux/macOS) têm mapeamento nomeado; qualquer
/// outro SO passa a grafia crua do Rust em vez de inventar uma string Node que não existe.
fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other, // "linux" já bate com o valor Node
    }
}

/// Espelha `process.arch` (Node) a partir de `std::env::consts::ARCH`.
fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        other => other, // "arm" já bate com o valor Node
    }
}

fn to_mb(bytes: u64) -> u64 {
    (bytes as f64 / 1024.0 / 1024.0).round() as u64
}

/// Machine-wide snapshot plus Mika's own RSS ("unknown" only if the OS cannot find our PID).
/// Blocking: enumerates processes and sleeps between the two CPU samples a load figure needs.
fn sample(start: Instant) -> Value {
    // Targeted refreshes: memory, CPU and Mika's own process — `System::new_all()` enumerated every
    // process (with CPU/disk each) plus disks/network/components for three numbers.
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();
    let own_pid = sysinfo::get_current_pid().ok();
    if let Some(pid) = own_pid {
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), false, ProcessRefreshKind::nothing().with_memory());
    }
    let own_memory = own_pid
        .and_then(|pid| system.process(pid).map(|process| json!(to_mb(process.memory()))))
        .unwrap_or_else(|| json!("unknown"));
    json!({
        "platform": node_platform(),
        "arch": node_arch(),
        "machine": {
            "memoryUsedMB": to_mb(system.used_memory()),
            "memoryTotalMB": to_mb(system.total_memory()),
            "cpuUsagePercent": (f64::from(system.global_cpu_usage()) * 10.0).round() / 10.0,
            "uptimeSeconds": System::uptime(),
        },
        "mikaProcess": { "memoryMB": own_memory, "uptimeSeconds": start.elapsed().as_secs() },
    })
}

pub(super) struct SystemInfoHandler {
    /// Capturado na montagem de `super::modules()`, bem cedo no boot — mesma aproximação honesta
    /// que `health.rs::HealthModule.start` já documenta para o mesmo conceito.
    start: Instant,
}

#[async_trait]
impl ToolHandler for SystemInfoHandler {
    async fn call(&self, _args: Value) -> Result<Value, String> {
        let start = self.start;
        let data = tokio::task::spawn_blocking(move || sample(start)).await.map_err(|error| error.to_string())?;
        Ok(json!({ "success": true, "data": data }))
    }
}

/// Espelha `inputSchema: z.object({})` — a tool não recebe nenhum argumento.
fn schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "get_system_info".into(),
        description: Some(DESCRIPTION.into()),
        input_schema: schema(),
        // TS registra com `pluginId: 'core'` (`this.register({...}, 'core')` em `init()`).
        plugin_id: "core".into(),
        requires_window: false,
    }
}

/// Instante do processo capturado uma única vez, na primeira (e única) chamada de
/// [`register`] — protege contra o mesmo módulo rodar `modules()` duas vezes num teste que chama
/// `crate::modules::all()` mais de uma vez no mesmo processo (o `Instant` do primeiro boot vence).
static START: OnceLock<Instant> = OnceLock::new();

pub(super) fn register() {
    let start = *START.get_or_init(Instant::now);
    registry().register(definition(), Arc::new(SystemInfoHandler { start }));
}

#[cfg(test)]
mod tests {
    use super::*;

    // "Como está meu PC?" got Mika's own 875MB RSS as a bare `memory`, read back as "875MB livres",
    // and the app's uptime as the machine's: the diagnosis was invented from the wrong process.
    #[tokio::test]
    async fn get_system_info_reports_the_machine_apart_from_mikas_own_process() {
        let handler = SystemInfoHandler { start: Instant::now() };
        let result = handler.call(json!({})).await.unwrap();
        assert_eq!(result["success"], json!(true));
        let data = &result["data"];
        assert!(data["platform"].is_string());
        assert!(data["arch"].is_string());
        assert!(data.get("memory").is_none() && data.get("uptime").is_none(), "no unlabeled figure: {data}");
        let machine = &data["machine"];
        let total = machine["memoryTotalMB"].as_u64().expect("total memory");
        let used = machine["memoryUsedMB"].as_u64().expect("used memory");
        assert!(total > 0 && used <= total, "{machine}");
        let cpu = machine["cpuUsagePercent"].as_f64().expect("cpu usage");
        assert!((0.0..=100.0).contains(&cpu), "{machine}");
        assert!(machine["uptimeSeconds"].as_u64().expect("machine uptime") > 0);
        let own = &data["mikaProcess"];
        assert!(own["memoryMB"].as_u64().is_some_and(|mb| mb > 0), "the targeted refresh must still find Mika's own process: {own}");
        assert!(own["uptimeSeconds"].is_u64(), "{own}");
    }

    #[test]
    fn node_platform_maps_windows_and_macos_to_the_node_spelling() {
        // Prova a TABELA de mapeamento sem depender do SO que roda o teste.
        assert_eq!(
            match "windows" { "windows" => "win32", "macos" => "darwin", other => other },
            "win32"
        );
        assert_eq!(
            match "macos" { "windows" => "win32", "macos" => "darwin", other => other },
            "darwin"
        );
    }

    #[test]
    fn node_arch_maps_x86_64_and_aarch64_to_the_node_spelling() {
        assert_eq!(
            match "x86_64" { "x86_64" => "x64", "x86" => "ia32", "aarch64" => "arm64", other => other },
            "x64"
        );
        assert_eq!(
            match "aarch64" { "x86_64" => "x64", "x86" => "ia32", "aarch64" => "arm64", other => other },
            "arm64"
        );
    }

    #[test]
    fn schema_declares_no_arguments_like_the_ts_zod_object() {
        assert_eq!(schema(), json!({ "type": "object", "properties": {} }));
    }

    #[test]
    fn definition_uses_the_core_plugin_id_like_ts_init() {
        assert_eq!(definition().plugin_id, "core");
        assert_eq!(definition().name, "get_system_info");
    }

    /// Positiva contra o `OnceLock` de processo — nunca nega presença de vizinho (`cargo test`
    /// roda em paralelo), só confirma que ESTA chamada de fato cadastra.
    #[test]
    fn register_registers_get_system_info_into_the_shared_process_wide_registry() {
        register();
        let all = registry().get_all();
        assert!(all.iter().any(|t| t.name == "get_system_info" && t.plugin_id == "core"));
    }
}
