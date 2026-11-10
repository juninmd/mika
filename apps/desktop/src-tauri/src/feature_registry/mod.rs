//! `FeatureRegistry` — a peça que falta para 4 dos 9 canais que ainda descem ao sidecar
//! (`mika:settings-declarations`, `mika:plugin-ui-catalog`, e os dois que dependem deles por
//! tabela: ver `docs/tauri-rust-migration.md`).
//!
//! Porta `packages/sdk/src/features/feature-registry.ts` (`FeatureRegistryService`): o registro
//! em RUNTIME de "o que cada plugin/módulo declarou" — settings, views e painéis — que hoje só
//! existe no processo Node porque `bootstrap/tools.ts::bootstrapTools` chama
//! `featureRegistry.register(...)` para os módulos de NÚCLEO (`brain`, `embeddings`/`llm`,
//! `voice`, `vision`, `browser`) e `PluginLoader::activate()` chama o mesmo `register()` para as
//! 9 features tardias, a cada boot. `mika:settings-declarations` lê `getAllSettings()` e
//! `mika:plugin-ui-catalog` lê `getUiCatalog()` — os dois getters que este arquivo reproduz.
//!
//! **Este módulo não reivindica canal nenhum e não entra em `native/mod.rs`.** Quem liga um canal
//! a ele decide isso depois — mas a INSTÂNCIA compartilhada já é entregue aqui, por
//! `feature_registry::shared()`, porque sem ela nenhum dos 8 módulos que vão chamar `register()`
//! tem como enxergar o que os outros registraram (a peça existiria e ficaria inútil).
//!
//! **Por que `static` + `OnceLock`, e não `ShellState` (o padrão do resto do crate para
//! singleton por processo — ver `state::config_store`).** `ConfigStore` PRECISA do
//! `AppHandle` porque resolve `app_data_dir()` — um caminho de disco que só existe depois que o
//! Tauri monta o app; por isso mora em `ShellState`, atrás de `config_store(app)`. Este registro
//! não toca disco nem rede: é `Vec` em memória, então não tem motivo para carregar `AppHandle`
//! por toda a cadeia de chamada só para chegar até ele. Um `static` de módulo dá a MESMA garantia
//! de instância única (ver `single_config_store_per_process` em `state.rs` para o custo de errar
//! isso) sem exigir `AppHandle` na assinatura de `register()` — e sem tocar `state.rs`, fora do
//! meu escopo nesta rodada. Se uma frente futura achar motivo concreto para mudar (ciclo de vida
//! por janela, reset em teste de integração que já usa `ShellState`), o tipo `FeatureRegistry` já
//! é independente do `static` que o guarda — trocar o acesso não pede tocar os outros métodos.
//!
//! **Sem lacuna de `AppHandle`, ao contrário de `ToolHandler::call`.** Este registro não executa
//! nada — ele só guarda e reserve `Declaration` (JSON) que o CALLER já montou. Quem monta um
//! `Declaration` roda no próprio código do módulo, ANTES de chamar `register()`, com qualquer
//! `AppHandle`/`ConfigStore` que já tiver à mão; `register()` em si não precisa de nenhum dos
//! dois porque não lê config nem invoca handler nenhum.
//!
//! ## i18n fica de fora, de propósito
//!
//! O lado TS NÃO localiza aqui: `FeatureRegistryService::getAllSettings()` devolve texto cru
//! (literal pt-BR ou chave `t()`), e é `ipc/settings.ts::registerSettingsIPC` quem chama
//! `localizeFeatureSettings()` depois, uma camada acima. Este módulo segue a MESMA divisão:
//! `get_all_settings()` devolve os `Declaration` exatamente como foram registrados. **Quem
//! reivindicar `mika:settings-declarations` em Rust tem de rodar a resolução de i18n antes de
//! responder** — sem isso a tela mostra a chave crua (`settings.voice.title`) em vez do título.
//! Portar `t()`/`TranslationDict` (`packages/sdk/src/i18n`, dividido por locale por causa do
//! teto de tamanho do Biome) não é desta peça: é trabalho de quem for ligar o canal, do mesmo
//! jeito que já é uma chamada separada do lado TS.
//!
//! ## Como um módulo de NÚCLEO registra
//!
//! Uma vez, no boot, com tudo que o módulo declara (equivalente a `registerFeatureWithTools` em
//! `bootstrap/tools.ts`, menos a parte de tools — ver `FeatureRegistration`):
//!
//! ```ignore
//! use crate::feature_registry::{FeatureRegistration, FeatureRegistry};
//!
//! registry.register(
//!     "mika.voice",
//!     FeatureRegistration {
//!         enabled: true,
//!         settings: vec![serde_json::json!({
//!             "tabId": "voice",
//!             "tabLabel": "settings.voice.title",
//!             "groups": [],
//!         })],
//!         ..Default::default()
//!     },
//! );
//! ```
//!
//! ## Como uma feature TARDIA registra
//!
//! Mesma chamada; `id` só quando a feature quer separar o id de settings do `pluginId` do
//! manifest (equivalente a `feature.id` em `FeatureDefinition`):
//!
//! ```ignore
//! registry.register(
//!     manifest.plugin_id.as_str(),
//!     FeatureRegistration {
//!         id: Some("gamer-companion.hud".into()),
//!         enabled: manifest.enabled,
//!         settings: panel_declarations,
//!         views: view_declarations,
//!         settings_panels: vec![],
//!     },
//! );
//! ```
//!
//! Chamar `register` duas vezes para o mesmo id (feature reativada, manifest recarregado) faz o
//! MESMO merge do lado TS: `settings` mescla por `tabId` (o registro anterior vence em empate),
//! `enabled` vira `true` se qualquer uma das duas chamadas pediu `true`, e `views`/
//! `settings_panels` são SUBSTITUÍDOS pela chamada mais recente — não mesclados. É o
//! `{...feature, tools: mergedTools, settings: mergedSettings}` de `feature-registry.ts`: só
//! `tools` (fora de escopo aqui) e `settings` entram na mescla explícita.

mod merge;
mod types;

#[cfg(test)]
mod tests;

pub use types::{Declaration, FeatureId, FeatureRegistration, PluginId, RegisteredFeature, UiCatalog};

use std::sync::{OnceLock, RwLock};

use merge::{inject_plugin_id, merge_settings, normalize_settings_declaration, remove_disabled_duplicates, tab_id_of};

static SHARED: OnceLock<FeatureRegistry> = OnceLock::new();

/// O `FeatureRegistry` do processo — o único que módulos de produção devem usar.
///
/// `FeatureRegistry::new()` continua `pub` para teste (cada teste quer um registro vazio e
/// isolado; ver `feature_registry::tests`), mas produção nunca chama — igual à regra de
/// `ConfigStore::open` em `state.rs`: duas instâncias são dois registros, e a metade dos módulos
/// registrando num e a metade no outro faria `mika:settings-declarations` devolver painel pela
/// metade, sem erro e sem log.
pub fn shared() -> &'static FeatureRegistry {
    SHARED.get_or_init(FeatureRegistry::new)
}

/// O registro compartilhado. Thread-safe via `RwLock` — leituras (`get_all_settings`,
/// `get_ui_catalog`) e escritas (`register`) podem vir de tarefas Tokio diferentes; o resto do
/// crate já paga esse custo em volta do `ort` com `spawn_blocking`, e este registro precisa da
/// mesma garantia sem herdar a complexidade de lá: não há chamada bloqueante aqui, só leitura e
/// clone de `Vec`/`Value`.
///
/// `Vec`, não `HashMap`, guarda as entradas. Dois motivos: `getAllSettings()` do lado TS itera um
/// `Map` na ORDEM DE INSERÇÃO — a mesma aba duplicada em duas features precisa resolver "quem
/// chegou primeiro" de forma determinística, e `HashMap` não garante ordem nenhuma entre rodadas.
/// E o registro é pequeno por natureza (dezenas de features, não milhares); uma busca linear por
/// `feature_id` custa menos do que o problema que resolveria.
#[derive(Default)]
pub struct FeatureRegistry {
    entries: RwLock<Vec<RegisteredFeature>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// `FeatureRegistryService::register`. Nunca falha: um lock envenenado (pânico em outra
    /// thread segurando o mesmo lock) é logado e a chamada vira no-op, em vez de propagar o
    /// pânico — o resto do app não trava porque um registro de UI não completou.
    pub fn register(&self, plugin_id: impl Into<PluginId>, registration: FeatureRegistration) {
        let plugin_id = plugin_id.into();
        let feature_id = registration
            .id
            .clone()
            .unwrap_or_else(|| FeatureId(plugin_id.0.clone()));

        let Ok(mut entries) = self.entries.write() else {
            log::error!("feature registry: lock envenenado, registro de '{feature_id}' descartado");
            return;
        };

        if registration.enabled && !registration.settings.is_empty() {
            remove_disabled_duplicates(&mut entries, &plugin_id, &feature_id, &registration.settings);
        }

        match entries.iter().position(|entry| entry.feature_id == feature_id) {
            Some(index) => {
                let previous = &entries[index];
                if previous.plugin_id != plugin_id {
                    log::warn!(
                        "feature registry: '{feature_id}' era de '{}', agora reivindicado por '{}'",
                        previous.plugin_id,
                        plugin_id
                    );
                }

                let settings = merge_settings(&previous.settings, &registration.settings);
                let enabled = registration.enabled || previous.enabled;

                entries[index] = RegisteredFeature {
                    feature_id,
                    plugin_id,
                    enabled,
                    settings,
                    views: registration.views,
                    settings_panels: registration.settings_panels,
                };
            }
            None => entries.push(RegisteredFeature {
                feature_id,
                plugin_id,
                enabled: registration.enabled,
                settings: registration.settings,
                views: registration.views,
                settings_panels: registration.settings_panels,
            }),
        }
    }

    /// `FeatureRegistryService::get`.
    #[cfg(test)]
    pub fn get(&self, feature_id: &FeatureId) -> Option<RegisteredFeature> {
        self.read_entries()
            .into_iter()
            .find(|entry| &entry.feature_id == feature_id)
    }

    /// `FeatureRegistryService::getAll`.
    pub fn get_all(&self) -> Vec<RegisteredFeature> {
        self.read_entries()
    }

    /// `FeatureRegistryService::getAllSettings`. **Não filtra por `enabled`** — mesma escolha do
    /// lado TS: uma feature desabilitada continua aparecendo na tela de Settings (é de lá que o
    /// usuário a reativa). `getAllViews`/`getAllSettingsPanels` são as que filtram.
    ///
    /// `tabId` duplicado entre features perde para o primeiro registrado, com aviso — mesma
    /// dedupe de `getAllSettings()`.
    pub fn get_all_settings(&self) -> Vec<Declaration> {
        let entries = self.read_entries();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();

        for entry in &entries {
            for declaration in &entry.settings {
                if let Some(tab_id) = tab_id_of(declaration) {
                    if !seen.insert(tab_id.to_string()) {
                        log::warn!(
                            "feature registry: tabId '{tab_id}' duplicado (plugin '{}') ignorado",
                            entry.plugin_id
                        );
                        continue;
                    }
                }
                out.push(normalize_settings_declaration(&entry.plugin_id, declaration));
            }
        }

        out
    }

    /// `FeatureRegistryService::getAllViews` — só features `enabled`.
    pub fn get_all_views(&self) -> Vec<Declaration> {
        self.read_entries()
            .iter()
            .filter(|entry| entry.enabled)
            .flat_map(|entry| entry.views.iter().map(|view| inject_plugin_id(&entry.plugin_id, view)))
            .collect()
    }

    /// `FeatureRegistryService::getAllSettingsPanels` — só features `enabled`.
    pub fn get_all_settings_panels(&self) -> Vec<Declaration> {
        self.read_entries()
            .iter()
            .filter(|entry| entry.enabled)
            .flat_map(|entry| {
                entry
                    .settings_panels
                    .iter()
                    .map(|panel| inject_plugin_id(&entry.plugin_id, panel))
            })
            .collect()
    }

    /// `FeatureRegistryService::getUiCatalog` — a forma exata que `mika:plugin-ui-catalog`
    /// devolve hoje pelo sidecar.
    pub fn get_ui_catalog(&self) -> UiCatalog {
        UiCatalog {
            views: self.get_all_views(),
            settings_panels: self.get_all_settings_panels(),
        }
    }

    fn read_entries(&self) -> Vec<RegisteredFeature> {
        match self.entries.read() {
            Ok(entries) => entries.clone(),
            Err(_) => {
                log::error!("feature registry: lock envenenado, leitura devolveu vazio");
                Vec::new()
            }
        }
    }
}
