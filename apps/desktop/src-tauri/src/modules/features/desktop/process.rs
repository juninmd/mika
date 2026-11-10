//! `action=process` — port de `listRunningProcesses`
//! (`apps/desktop/src/main/infrastructure/os/window-process-list.ts`), via `sysinfo` (já
//! dependência do crate). **Melhor que o original**: o TS só lista processos no Windows (via
//! `powershell -EncodedCommand`, ver `INVENTORY_SCRIPT`); aqui funciona nos três SOs — o próprio
//! contrato da tool já promete isso ("lists the heaviest running processes"), a limitação era do
//! script PowerShell, não do produto.

use serde_json::{json, Value};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Espelha `RunningProcess` (`window-process-list.ts`): nome, pid, memória em MB. Top 25 por
/// working set, igual ao `Select-Object -First 25` do script original.
pub(super) fn list_running_processes() -> Value {
    // Snapshot novo a cada chamada, sem cache entre invocações — mesma política do TS (que dispara
    // um processo `powershell` novo a cada leitura), só que sem o processo filho. Only what the
    // answer uses (each process's memory): `System::new_all()` also sampled CPU, disk and every
    // component/network/disk list for a tool call that reads none of it.
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());

    let mut processes: Vec<(String, u32, f64)> = system
        .processes()
        .values()
        .map(|process| {
            let name = process.name().to_string_lossy().into_owned();
            let pid = process.pid().as_u32();
            let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
            (name, pid, memory_mb)
        })
        .collect();

    processes.sort_by(|a, b| b.2.total_cmp(&a.2));
    processes.truncate(25);

    let list: Vec<Value> = processes
        .into_iter()
        .map(|(name, pid, memory_mb)| json!({ "name": name, "pid": pid, "memoryMB": memory_mb.round() }))
        .collect();

    json!({ "success": true, "data": { "processes": list, "count": list.len() } })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: a lista é real (o próprio processo de teste aparece nela), limitada a 25, e
    /// ordenada por memória decrescente.
    #[test]
    fn list_running_processes_returns_a_real_bounded_sorted_snapshot() {
        let result = list_running_processes();
        assert_eq!(result["success"], json!(true));
        let processes = result["data"]["processes"].as_array().expect("processes deveria ser array");
        assert!(!processes.is_empty(), "esperava pelo menos um processo no sistema de teste");
        assert!(processes.len() <= 25);

        for pair in processes.windows(2) {
            let a = pair[0]["memoryMB"].as_f64().unwrap();
            let b = pair[1]["memoryMB"].as_f64().unwrap();
            assert!(a >= b, "lista não está ordenada por memória decrescente");
        }
    }
}
