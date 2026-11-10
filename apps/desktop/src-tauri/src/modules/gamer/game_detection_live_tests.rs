//! Processos REAIS (cópia de um binário inofensivo renomeada para o exe do jogo) passando pela
//! mesma listagem que o watcher usa — prova o caminho `sysinfo` → `normalize` → `known_games`, que
//! os testes com `vec!["valorant"]` fabricado não exercitam.

use super::game_detection::{find_active_game, list_process_names, running_processes};
use std::process::{Child, Command, Stdio};

/// (exe como aparece no Gerenciador de Tarefas, título esperado)
const FAKE_GAMES: &[(&str, &str)] = &[
    ("eldenring.exe", "Elden Ring"),
    ("Cyberpunk2077.exe", "Cyberpunk 2077"),
    ("VALORANT-Win64-Shipping.exe", "Valorant"),
    ("StellarBlade.exe", "Stellar Blade"),
    ("hades2.exe", "Hades II"),
    ("cs2.exe", "Counter-Strike 2"),
];
const DECOYS: &[&str] = &["notepad2.exe", "SecurityHealthSystray.exe", "chrome.exe"];

#[cfg(windows)]
fn host_binary() -> (std::path::PathBuf, &'static [&'static str]) {
    (std::path::PathBuf::from(r"C:\Windows\System32\ping.exe"), &["-n", "40", "127.0.0.1"])
}
#[cfg(not(windows))]
fn host_binary() -> (std::path::PathBuf, &'static [&'static str]) {
    (std::path::PathBuf::from("/bin/sleep"), &["40"])
}

struct Fake(Child);
impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.0.kill();
        // Sem `wait`, o processo morto vira zumbi, segue na lista de processos e casa com o jogo seguinte.
        let _ = self.0.wait();
    }
}

fn spawn_as(dir: &std::path::Path, exe: &str) -> Fake {
    let (src, args) = host_binary();
    let dst = dir.join(exe);
    std::fs::copy(&src, &dst).expect("copy host binary");
    // `ETXTBSY` (os error 26): outro teste faz `fork` enquanto o arquivo recém-copiado ainda está aberto
    // para escrita nesta thread; o `exec` seguinte é recusado por um instante. Tentar de novo resolve.
    let mut attempts = 0;
    loop {
        match Command::new(&dst).args(args).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => return Fake(child),
            Err(error) if error.raw_os_error() == Some(26) && attempts < 50 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(error) => panic!("spawn fake game: {error}"),
        }
    }
}

/// Espera (com prazo, não `sleep` fixo) o SO listar o processo `pid` e devolve os nomes que o
/// watcher veria DESTE processo apenas. Filtrar por PID é o que isola a execução: outra
/// `cargo test` rodando na mesma máquina também sobe `eldenring.exe`, e o retrato inteiro
/// reconheceria o jogo dela como se fosse o nosso (ou chamaria de "jogo" um chamariz).
fn names_of(pid: u32) -> Vec<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let own: Vec<String> = running_processes().into_iter().filter(|(p, _)| *p == pid).map(|(_, key)| key).collect();
        if !own.is_empty() || std::time::Instant::now() >= deadline {
            return own;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn real_fake_game_processes_are_recognised_one_at_a_time_and_decoys_are_not() {
    let dir = std::env::temp_dir().join(format!("mika-fake-games-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();

    for decoy in DECOYS {
        let process = spawn_as(&dir, decoy);
        if let Some((info, _)) = find_active_game(&names_of(process.0.id()), &[]) {
            failures.push(format!("decoy {decoy} recognised as {}", info.name));
        }
    }
    for (exe, expected) in FAKE_GAMES {
        let process = spawn_as(&dir, exe);
        let pid = process.0.id();
        match find_active_game(&names_of(pid), &[]) {
            Some((info, _)) if info.name == *expected => {}
            other => failures.push(format!("{exe}: expected {expected}, got {:?}", other.map(|(i, _)| i.name))),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A listagem completa que o watcher usa enxerga um processo real e não repete nomes — o que o
/// antigo smoke "não vazia e deduplicada" só aproximava sem olhar para nada em particular.
#[test]
fn the_full_snapshot_lists_a_real_process_once() {
    let dir = std::env::temp_dir().join(format!("mika-fake-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Nome curto de propósito: o Linux trunca o nome do processo (`comm`) em 15 caracteres.
    let process = spawn_as(&dir, "snapprobe.exe");
    assert!(!names_of(process.0.id()).is_empty(), "o SO nunca listou o processo de teste");

    let names = list_process_names();
    assert_eq!(names.iter().filter(|name| name.as_str() == "snapprobe").count(), 1, "visível uma vez só");
    let unique: std::collections::HashSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "snapshot deduplicado");
    drop(process);
    let _ = std::fs::remove_dir_all(&dir);
}
