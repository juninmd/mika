//! Port de `services/steam-local-stats.ts` (metade "arte da biblioteca") — separado de
//! `steam_local_stats.rs` só para caber no limite de 300 linhas por arquivo; mesmo arquivo TS de
//! origem. Escada poster -> header -> logo -> ícone, com fallback para o ícone legado.

use serde::Serialize;
use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SteamArtKind {
    Poster,
    Header,
    Logo,
    Icon,
}

impl SteamArtKind {
    fn max_bytes(self) -> u64 {
        match self {
            SteamArtKind::Poster => 200 * 1024,
            SteamArtKind::Header => 200 * 1024,
            SteamArtKind::Logo => 160 * 1024,
            SteamArtKind::Icon => 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SteamGameArt {
    #[serde(rename = "dataUri")]
    pub data_uri: String,
    pub kind: SteamArtKind,
}

fn to_data_uri(path: &Path, kind: SteamArtKind) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > kind.max_bytes() {
        return None;
    }
    let mime = if path.to_string_lossy().to_lowercase().ends_with(".png") { "image/png" } else { "image/jpeg" };
    let bytes = fs::read(path).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

/// Procura `<base>_brazilian<ext>` e `<base><ext>` na raiz do diretório e em UM nível de
/// subdiretórios — mesma profundidade do TS.
fn find_named_art(dir: &Path, base_name: &str, extension: &str) -> Option<PathBuf> {
    let localized = format!("{base_name}_brazilian{extension}");
    let plain = format!("{base_name}{extension}");
    let mut candidates = vec![dir.join(&localized), dir.join(&plain)];

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                candidates.push(entry.path().join(&localized));
                candidates.push(entry.path().join(&plain));
            }
        }
    }
    candidates.into_iter().find(|c| c.exists())
}

fn find_smallest_icon(dir: &Path) -> Option<PathBuf> {
    let re = regex::Regex::new(r"(?i)^[0-9a-f]{40}\.jpg$").expect("static regex");
    let entries = fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| re.is_match(n)))
        .filter_map(|e| fs::metadata(e.path()).ok().map(|m| (e.path(), m.len())))
        .min_by_key(|(_, size)| *size)
        .map(|(path, _)| path)
}

/// Escada poster -> header -> logo -> ícone, com fallback para o ícone legado
/// `<appId>_icon.jpg` fora da pasta de cache por jogo.
pub fn read_game_art(steam_root: &Path, app_id: &str) -> Option<SteamGameArt> {
    let dir = steam_root.join("appcache").join("librarycache").join(app_id);
    if dir.exists() {
        let ladder: [(SteamArtKind, Option<PathBuf>); 4] = [
            (SteamArtKind::Poster, find_named_art(&dir, "library_600x900", ".jpg")),
            (SteamArtKind::Header, find_named_art(&dir, "header", ".jpg")),
            (SteamArtKind::Logo, find_named_art(&dir, "logo", ".png")),
            (SteamArtKind::Icon, find_smallest_icon(&dir)),
        ];
        for (kind, path) in ladder {
            if let Some(path) = path {
                if let Some(data_uri) = to_data_uri(&path, kind) {
                    return Some(SteamGameArt { data_uri, kind });
                }
            }
        }
    }

    let legacy = steam_root.join("appcache").join("librarycache").join(format!("{app_id}_icon.jpg"));
    if legacy.exists() {
        if let Some(data_uri) = to_data_uri(&legacy, SteamArtKind::Icon) {
            return Some(SteamGameArt { data_uri, kind: SteamArtKind::Icon });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;

    #[test]
    fn reads_game_art_as_a_base64_data_uri_within_the_size_cap() {
        let scratch = ScratchDir::new("art");
        let cache_dir = scratch.path().join("appcache").join("librarycache").join("10");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("header.jpg"), b"fake-jpeg-bytes").unwrap();

        let art = read_game_art(scratch.path(), "10").unwrap();
        assert_eq!(art.kind, SteamArtKind::Header);
        assert!(art.data_uri.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn oversized_art_is_skipped_not_truncated() {
        let scratch = ScratchDir::new("art-oversized");
        let cache_dir = scratch.path().join("appcache").join("librarycache").join("10");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("header.jpg"), vec![0u8; 200 * 1024 + 1]).unwrap();

        assert!(read_game_art(scratch.path(), "10").is_none());
    }

    #[test]
    fn falls_back_to_the_legacy_flat_icon_file() {
        let scratch = ScratchDir::new("art-legacy");
        let cache_root = scratch.path().join("appcache").join("librarycache");
        fs::create_dir_all(&cache_root).unwrap();
        fs::write(cache_root.join("10_icon.jpg"), b"fake-icon").unwrap();

        let art = read_game_art(scratch.path(), "10").unwrap();
        assert_eq!(art.kind, SteamArtKind::Icon);
    }

    #[test]
    fn no_art_anywhere_returns_none() {
        let scratch = ScratchDir::new("art-none");
        assert!(read_game_art(scratch.path(), "10").is_none());
    }
}
