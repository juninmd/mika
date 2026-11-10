//! Os 7 stores que ganharam backing Rust em 2026-09-22 — extraído de `clear.rs` só pelo limite de
//! 300 linhas do arquivo (estava em 276, ver o cabeçalho dele), não por camada nova.
//! [`ExtraStores`] agrupa os campos/passos que `clear.rs::wipe_everything` precisa para
//! `person catalog`/`social memory`/`game sessions`/`screen captures`/`safe secrets`/`log files`/
//! `exported data archives` — a tabela de paridade com o TS mora no cabeçalho de `clear.rs`, não
//! duplicada aqui.

use std::path::PathBuf;
#[cfg(test)]
use std::path::Path;

use tauri::AppHandle;

use crate::modules::features::person_catalog::{db_path as catalog_db_path, CatalogStore};
use crate::modules::gamer::session_db::{db_path as game_sessions_db_path, SessionDb};
use crate::modules::memory::social::SocialMemoryService;
use crate::modules::memory::MemoryStore;

use super::wipe_files;

/// Direto em `<userData>/captures`, SEM subpasta de plugin.
///
/// A primeira versão desta linha era `plugins/mika.vision/captures` — plausível pelo nome do plugin,
/// e errada. `vision/index.ts` resolve a pasta como `join(ctx.dataDir, 'captures')`, e o `ctx` do
/// vision vem de `createCorePluginContext`, que recebe `app.getPath('userData')` PURO
/// (`mika-services.ts:42` → `bootstrap/vision.ts:8`): não há subpasta por plugin neste caminho.
///
/// **O defeito não falhava em lugar nenhum**, e é por isso que ele merece constante e teste: varrer
/// um diretório inexistente devolve `0` removido — não erro —, `wipe` empurrava `"screen captures"`
/// para `cleared` do mesmo jeito, e o usuário lia "capturas de tela apagadas" com as capturas
/// intactas no disco. Resposta que significa "não fiz" é pior aqui do que em qualquer outra tool:
/// a promessa desta é justamente que o dado sumiu.
const VISION_CAPTURES_DIR: &str = "captures";

pub(super) struct ExtraStores {
    catalog: &'static CatalogStore,
    game_sessions: SessionDb,
    person_images_dir: PathBuf,
    vision_captures_dir: PathBuf,
    logs_dir: PathBuf,
    exports_dir: PathBuf,
    safe_secrets_path: PathBuf,
    /// Pasta de dados do perfil: `usage-habits.json` mora direto nela.
    data_dir: PathBuf,
    backups_dir: PathBuf,
    reports_dir: PathBuf,
}

impl ExtraStores {
    /// Produção: `catalog` vem do SINGLETON de processo (`person_catalog::catalog_store`), nunca de
    /// uma abertura própria — ver o cabeçalho de `person_catalog/store_singleton.rs` para o porquê
    /// (duas instâncias sobre o MESMO `catalog.json` fariam a próxima escrita de uma desfazer o
    /// `clear_all()` da outra). `game_sessions` pode ser uma conexão própria com segurança: é
    /// SQLite (WAL), não um índice em memória — mesmo raciocínio que já vale para `SchedulerStore`.
    pub(super) fn open(app: &AppHandle) -> Result<Self, String> {
        let catalog = crate::modules::features::person_catalog::catalog_store(app)?;
        let game_sessions = SessionDb::open(&game_sessions_db_path(app)?)?;
        let data_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
        let person_images_dir =
            catalog_db_path(app)?.parent().ok_or("catalog.json sem diretório pai")?.join("person-images");

        Ok(Self {
            catalog,
            game_sessions,
            person_images_dir,
            vision_captures_dir: data_dir.join(VISION_CAPTURES_DIR),
            logs_dir: data_dir.join("logs"),
            exports_dir: data_dir.join("exports"),
            safe_secrets_path: data_dir.join("mika-safe-secrets.json"),
            backups_dir: data_dir.join("backups"),
            reports_dir: data_dir.join(crate::modules::chat::report::REPORTS_DIR),
            data_dir,
        })
    }

    /// Teste: um `CatalogStore` PRÓPRIO (`Box::leak`, isolado por `TempDir`), nunca o singleton de
    /// processo — mesmo truque que `MemoryStore` já usa em `clear.rs` para satisfazer `&'static`.
    #[cfg(test)]
    pub(super) fn open_for_test(dir: &Path) -> Self {
        let catalog: &'static CatalogStore = Box::leak(Box::new(
            CatalogStore::open(&dir.join("plugins").join("mika.person-catalog").join("catalog.json")).unwrap(),
        ));
        Self {
            catalog,
            game_sessions: SessionDb::open(&dir.join("gamer-sessions.db")).unwrap(),
            person_images_dir: dir.join("plugins").join("mika.person-catalog").join("person-images"),
            vision_captures_dir: dir.join(VISION_CAPTURES_DIR),
            logs_dir: dir.join("logs"),
            exports_dir: dir.join("exports"),
            safe_secrets_path: dir.join("mika-safe-secrets.json"),
            data_dir: dir.to_path_buf(),
            backups_dir: dir.join("backups"),
            reports_dir: dir.join(crate::modules::chat::report::REPORTS_DIR),
        }
    }

    /// Tenta os 7 stores, acumulando em `cleared`/`failed` — MESMO contrato que
    /// `clear.rs::wipe_everything` já usa para os 4 originais. `mem` é o `MemoryStore` de processo
    /// que `ClearHistoryHandler` já segura (a memória social vive no MESMO arquivo que a episódica,
    /// ver o cabeçalho de `modules::memory::social`), por isso entra como parâmetro em vez de campo.
    pub(super) fn wipe(&self, mem: &MemoryStore, cleared: &mut Vec<&'static str>, failed: &mut Vec<(&'static str, String)>) {
        // Fotos primeiro (melhor esforço, nunca falha o passo): espelha
        // `PersonCatalogService.clearAll`, que loga um aviso e segue em frente se
        // `imageCache.clearAll()` falhar, em vez de abortar o wipe do catálogo por isso.
        wipe_files::clear_dir_all(&self.person_images_dir);
        if let Some(catalog_dir) = self.person_images_dir.parent() {
            wipe_files::clear_catalog_backups(catalog_dir);
        }
        match self.catalog.clear_all() {
            Ok(_) => cleared.push("person catalog"),
            Err(error) => failed.push(("person catalog", error)),
        }

        match SocialMemoryService::new(mem).clear_all() {
            Ok(_) => cleared.push("social memory"),
            Err(error) => failed.push(("social memory", error)),
        }

        match self.game_sessions.clear_all() {
            Ok(_) => cleared.push("game sessions"),
            Err(error) => failed.push(("game sessions", error)),
        }

        // As 3 varreduras de arquivo abaixo nunca falham (`wipe_files::clear_dir_all`/
        // `clear_log_files`/`clear_exported_archives` devolvem contagem, nunca `Result`; diretório
        // ausente já é `0` removido, não erro — ver o cabeçalho de `wipe_files.rs`).
        wipe_files::clear_dir_all(&self.vision_captures_dir);
        cleared.push("screen captures");

        match wipe_files::clear_safe_secrets(&self.safe_secrets_path) {
            Ok(_) => cleared.push("safe secrets"),
            Err(error) => failed.push(("safe secrets", error)),
        }

        wipe_files::clear_log_files(&self.logs_dir);
        cleared.push("log files");

        wipe_files::clear_exported_archives(&self.exports_dir);
        cleared.push("exported data archives");

        // O que o app guarda por conta própria e o usuário nunca pediu para guardar: o contador de
        // programas em foco (`usage-habits.json`, ligado por padrão), as cópias pré-atualização
        // (`backups/pre-*`: chat, memória e perfil inteiros) e os relatórios "reportar resposta".
        match crate::modules::features::desktop::habits::clear_all(&self.data_dir) {
            Ok(()) => cleared.push("usage habits"),
            Err(error) => failed.push(("usage habits", error)),
        }
        wipe_files::clear_prefixed_dirs(&self.backups_dir, "pre-");
        cleared.push("update backups");
        wipe_files::clear_dir_all(&self.reports_dir);
        cleared.push("response reports");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::person_catalog::types::UpsertInput;
    use crate::modules::gamer::session_types::GameSession;
    use crate::modules::memory::social::{AddFactInput, SocialMemoryService as Social};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-privacy-clear-ext-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn game_session(id: &str) -> GameSession {
        GameSession {
            id: id.into(),
            game_id: "g1".into(),
            game_name: "Hades".into(),
            start_time_ms: 1000,
            end_time_ms: None,
            events: vec![],
            summary: None,
        }
    }

    /// Prova, arquivo a arquivo e linha a linha, que os 7 stores recém-cobertos de fato somem —
    /// não só que a chave aparece em `cleared`.
    #[test]
    fn wipe_actually_empties_all_ten_stores_and_names_them_cleared() {
        let dir = TempDir::new();
        let extra = ExtraStores::open_for_test(&dir.0);
        let mem = MemoryStore::open_in_memory().unwrap();

        std::fs::create_dir_all(&extra.person_images_dir).unwrap();
        std::fs::write(extra.person_images_dir.join("pc_deadbeef.png"), b"foto fabricada").unwrap();
        std::fs::create_dir_all(&extra.vision_captures_dir).unwrap();
        std::fs::write(extra.vision_captures_dir.join("captura.png"), b"captura fabricada").unwrap();
        std::fs::create_dir_all(&extra.logs_dir).unwrap();
        std::fs::write(extra.logs_dir.join("mika-human.log"), b"log fabricado").unwrap();
        std::fs::create_dir_all(&extra.exports_dir).unwrap();
        std::fs::write(extra.exports_dir.join("1700000000000.zip"), b"zip fabricado").unwrap();
        std::fs::write(&extra.safe_secrets_path, r#"{"svc:acc":"cifrado"}"#).unwrap();
        std::fs::write(extra.data_dir.join("usage-habits.json"), b"{}").unwrap();
        std::fs::create_dir_all(extra.backups_dir.join("pre-1.0.0")).unwrap();
        std::fs::write(extra.backups_dir.join("pre-1.0.0").join("chat.db"), b"copia do chat").unwrap();
        std::fs::create_dir_all(&extra.reports_dir).unwrap();
        std::fs::write(extra.reports_dir.join("1700000000000.json"), b"{}").unwrap();
        // Catálogo corrompido já virou `catalog.json.corrupt-<ms>.bak` (nomes de pessoas em claro).
        let stale_backup = extra.person_images_dir.parent().unwrap().join("catalog.json.corrupt-1700000000000.bak");
        std::fs::write(&stale_backup, b"{\"persons\":[{\"name\":\"Ada\"}]}").unwrap();
        extra.catalog.upsert(UpsertInput { name: "Ada".into(), ..Default::default() }, 1000, 100).unwrap();
        extra.game_sessions.insert(&game_session("s1")).unwrap();
        Social::new(&mem).add_fact(true, AddFactInput { fact: "gosta de rust".into(), ..Default::default() }, 1).unwrap();

        let mut cleared = Vec::new();
        let mut failed = Vec::new();
        extra.wipe(&mem, &mut cleared, &mut failed);

        assert!(failed.is_empty(), "nenhum dos 7 devia falhar: {failed:?}");
        for expected in [
            "person catalog",
            "social memory",
            "game sessions",
            "screen captures",
            "safe secrets",
            "log files",
            "exported data archives",
            "usage habits",
            "update backups",
            "response reports",
        ] {
            assert!(cleared.contains(&expected), "'{expected}' não está em cleared: {cleared:?}");
        }
        assert!(!stale_backup.exists(), "backup .bak do catálogo corrompido (nomes de pessoas) sobreviveu ao wipe");
        assert!(!extra.data_dir.join("usage-habits.json").exists(), "hábitos de uso sobreviveram ao wipe");
        assert!(!extra.backups_dir.join("pre-1.0.0").exists(), "backup pré-update (cópia do chat) sobreviveu ao wipe");
        assert!(std::fs::read_dir(&extra.reports_dir).unwrap().next().is_none(), "relatório sobreviveu ao wipe");

        assert!(std::fs::read_dir(&extra.person_images_dir).unwrap().next().is_none(), "foto de pessoa sobreviveu ao wipe");
        assert!(std::fs::read_dir(&extra.vision_captures_dir).unwrap().next().is_none(), "captura de tela sobreviveu ao wipe");
        assert!(!extra.logs_dir.join("mika-human.log").exists(), "arquivo de log sobreviveu ao wipe");
        assert!(!extra.exports_dir.join("1700000000000.zip").exists(), "arquivo exportado sobreviveu ao wipe");
        assert!(!extra.safe_secrets_path.exists(), "arquivo de credenciais sobreviveu ao wipe");
        assert!(extra.catalog.list(None, None, 20).is_empty(), "catálogo de pessoas sobreviveu ao wipe");
        assert!(extra.game_sessions.history(10).unwrap().is_empty(), "sessão de jogo sobreviveu ao wipe");
        assert!(Social::new(&mem).get_graph().unwrap().facts.is_empty(), "fato social sobreviveu ao wipe");
    }

    /// O caminho das capturas vem do FONTE que as escreve, não da suposição de que todo plugin tem
    /// subpasta própria.
    ///
    /// Nenhum teste de comportamento pega este erro: o fixture deriva a pasta da MESMA constante que
    /// a produção, então um caminho errado continua verde de ponta a ponta — e em produção varre um
    /// diretório inexistente, devolve `0` sem erro e ainda assim reporta "screen captures" apagadas.
    /// `vision/index.ts`, `bootstrap/vision.ts` e `mika-services.ts` foram removidos (backend Node
    /// deletado na migração para Tauri) sem twin sobrevivente — e a busca por um escritor de
    /// capturas em disco no `modules/vision/**` atual não achou nenhum (`fs::write`/`File::create`
    /// sobre PNG/JPEG): hoje o Rust não persiste captura de tela, só passa direto pro LLM. Ou seja,
    /// o oráculo independente que provava "a pasta que o wipe varre é a mesma que o produtor
    /// escreve" deixou de existir dos dois lados — não só o TS morreu, o produtor Rust equivalente
    /// nunca chegou a existir. **Risco residual documentado, não escondido**: se um escritor de
    /// capturas for adicionado ao Rust, ele PRECISA usar `data_dir.join(VISION_CAPTURES_DIR)`, e
    /// merece voltar a ter aqui um teste cruzado contra esse produtor. Por ora, o teste fica restrito
    /// ao que ainda pode ser verificado sem inventar um lado que não existe: a própria produção
    /// deste arquivo continua montando o caminho sem subpasta de plugin.
    #[test]
    fn the_captures_dir_has_no_plugin_subfolder_segment() {
        // As duas asserções abaixo provam o lado RUST, e continuariam
        // verdes com a produção compondo `plugins/mika.vision/captures` de novo — que é exatamente
        // o defeito que existia. A produção monta o caminho a partir da constante, direto sobre o
        // `data_dir`, sem nenhum segmento no meio.
        // Só CÓDIGO: o comentário da constante acima cita `mika.vision` em prosa de propósito, para
        // explicar o defeito. Gate que casa a string crua acusaria justamente quem documentou —
        // mesmo corte que `state.rs::single_config_store_per_process` já faz, pela mesma razão.
        // O corte é em `mod tests {`, NÃO em `#[cfg(test)]`: este arquivo tem um `#[cfg(test)] use`
        // na linha 9, e cortar ali deixaria `production` com nove linhas — o teste passaria a ler
        // quase nada e viraria verde por vacuidade. Foi o que aconteceu ao escrevê-lo.
        let source = include_str!("clear_ext.rs");
        let production: String = source
            .split("mod tests {")
            .next()
            .unwrap_or_default()
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            production.contains("vision_captures_dir: data_dir.join(VISION_CAPTURES_DIR)"),
            "a produção deixou de montar o caminho das capturas direto sobre o data_dir"
        );
        assert!(
            !production.contains("mika.vision"),
            "voltou a subpasta de plugin no caminho das capturas — o vision do núcleo não tem uma (ver a constante)"
        );
    }

    /// NEGATIVO — diretórios ausentes (primeira instalação, nunca gerou captura/log/export/foto)
    /// não podem virar `failed`: `wipe_files` devolve `0`, não erro, para diretório inexistente.
    #[test]
    fn wipe_on_a_fresh_install_with_no_files_yet_fails_nothing() {
        let dir = TempDir::new();
        let extra = ExtraStores::open_for_test(&dir.0);
        let mem = MemoryStore::open_in_memory().unwrap();

        let mut cleared = Vec::new();
        let mut failed = Vec::new();
        extra.wipe(&mem, &mut cleared, &mut failed);

        assert!(failed.is_empty(), "diretório ausente não pode virar falha: {failed:?}");
        assert_eq!(cleared.len(), 10);
    }
}
