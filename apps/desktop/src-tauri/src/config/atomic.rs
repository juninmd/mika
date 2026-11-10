//! Escrita atômica de arquivo pequeno de estado (config, hábitos, necessidades, catálogo de pessoas).
//!
//! `fs::write` direto trunca o destino ANTES de gravar: um crash no meio deixa o arquivo vazio ou
//! pela metade e o próximo boot lê lixo. Aqui: temporário ao lado do destino + `sync_all` (os dados
//! chegam ao disco antes do rename; sem isso uma queda de energia pode deixar o arquivo novo vazio)
//! + `rename`, que o leitor enxerga como "arquivo antigo" ou "arquivo novo", nunca meio.
//!
//! O temporário é `<arquivo>.tmp`, fixo: quem chama serializa as próprias gravações (todos seguram
//! um `Mutex` do estado ao gravar).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    let temporary = PathBuf::from(name);

    let result = fs::File::create(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-atomic-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_the_target_and_leaves_no_temporary() {
        let dir = scratch("replace");
        let target = dir.join("state.json");
        fs::write(&target, "antigo").unwrap();

        write_atomic(&target, b"novo").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "novo");
        assert!(!dir.join("state.json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Falha no meio (aqui: o destino é um diretório, o `rename` recusa) não toca o que havia e não
    /// deixa o temporário para trás.
    #[test]
    fn a_failed_write_keeps_the_old_file_and_cleans_up() {
        let dir = scratch("fail");
        let target = dir.join("state.json");
        fs::create_dir(&target).unwrap();

        assert!(write_atomic(&target, b"novo").is_err());

        assert!(target.is_dir());
        assert!(!dir.join("state.json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
