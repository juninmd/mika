//! `ToolRegistry` nativo — port de `ToolRegistryService` (`packages/sdk/src/tools/tool-registry.ts`).
//!
//! O REGISTRO: nomear, catalogar, validar contra o JSON Schema declarado (`validation.rs`) e
//! despachar para um handler nativo. Desde 2026-09-21d também carrega o gate de aprovação
//! (`approval.rs`, port de `ApprovalGateService`) e o campo `destructive` — ver §"Gate de aprovação"
//! abaixo. Ficam de fora, ainda: `hostId`/`unregisterHostTools` (MCP host — `modules/mcp` já tem seu
//! próprio ciclo de vida de conexão) e a tool `get_system_info` que o `init()` do TS registra
//! sozinha (ainda só existe lá — ver o teste `get_system_info_is_still_ts_only` em `mod.rs`).
//!
//! **Ponto de extensão** — um módulo já portado (`vision`, `browser`, `voice`, `memory`, …) se
//! registra chamando, no próprio `modules()`/setup:
//! ```ignore
//! crate::modules::tools::registry().register(
//!     ToolDefinition { name: "...".into(), description: Some("...".into()),
//!                      input_schema: json!({ "type": "object", ... }), plugin_id: "vision".into(),
//!                      requires_window: false },
//!     Arc::new(MyHandler { /* o que precisar, capturado aqui */ }),
//! );
//! ```
//! `ToolHandler::call` não recebe `AppHandle` — mesma escolha de `telemetry.rs`/`voice.rs` para não
//! acoplar um caminho testável a um app Tauri inteiro; quem precisar de `AppHandle`/config fecha
//! sobre a própria referência no construtor do handler.
//!
//! Reverificado por grep em `src-tauri/src` em 2026-09-21d (`rg "tools::registry\(\)\.register"`):
//! isto MUDOU desde a rodada anterior. `gamer::register_tools` (`track_gamer_mood`, `steam`),
//! `features::mood`/`features::desktop` (`mood`, `desktop`) e `vision::register` (`vision_capture`,
//! `vision_read`, chamada em produção por `modules::register_config_bound_tools`, ela mesma chamada
//! por `lib.rs::setup`) registram de verdade fora de teste — 6 tools reais hoje, não mais zero. O
//! catálogo deixou de ser LITERALMENTE vazio. Continua, porém, muito mais estreito que o catálogo TS
//! (`bootstrap/tools.ts::registerFeatureWithTools` para `mika.brain`/`mika.embeddings`/`mika.voice`/
//! `mika.vision`/`mika.browser`, mais as 9 features tardias via `extensions.registerTool` e o
//! `get_system_info` automático) — `mod.rs` documenta por que isso ainda barra
//! `mika:tool-execute`/`mika:tools-list`/`-by-plugin`.
//!
//! ## Gate de aprovação — por que entrou agora
//!
//! `modules/mcp/register.rs` registra tool de servidor MCP de terceiro e nega toda chamada, sempre,
//! HARDCODED, porque não havia gate nenhum aqui — infraestrutura sem consumidor era a recusa certa
//! na rodada anterior. Mudou: `brain::turn` já executa `tools::registry().execute(...)` em produção
//! (via `mika:chat-send`, já nativo), e o que estiver registrado aqui — incluindo uma tool MCP que um
//! dia troque para `register_destructive` — passa por este mesmo `execute()`. O gate PRECISA existir
//! no ponto de despacho, não em cada chamador. [`ToolRegistry::register_destructive`] marca uma tool
//! como `destructive` (mesmo sentido do campo homônimo em `ToolDef`/`tool-registry.ts`);
//! [`ToolRegistry::execute`] checa essa marca ANTES de despachar e, se destrutiva, pede aprovação via
//! [`super::approval::ApprovalGateService`] — sem handler configurado (nenhum tem `AppHandle` para
//! montar um ainda), nega por padrão, mesmo fail-closed que `mcp/register.rs` já tinha, só que agora
//! aplicado no lugar certo. `execute()`/os testes deste arquivo são o consumidor real: provam o
//! caminho ponta a ponta sem precisar editar `modules/mcp/` (fora do meu escopo nesta rodada — quem
//! for dono daquele arquivo troca `.register(...)` por `.register_destructive(...)`, uma linha).

use async_trait::async_trait;
use futures::FutureExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, OnceLock, RwLock};

use super::approval::{ApprovalGateService, ApprovalHandler, ApprovalRequest};
use super::validation::{coerce_args, validate_args};

/// Quem pediu a chamada — decide qual política de aprovação vale (`broker.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOrigin {
    /// O modelo escolhendo a ação no loop de chat.
    Llm,
    /// O usuário, por um clique da UI (`mika:tool-execute`); o IPC já aplicou a política dele.
    User,
    /// Código do próprio app (mood, atalhos): confiável, sem risco por ação.
    System,
}

mod broker;

/// Handler de uma tool nativa. Um módulo por trás de cada implementação; ver o ponto de extensão
/// no comentário do módulo.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, args: Value) -> Result<Value, String>;
}

/// Espelha o subconjunto de `ToolDef` (`define-tool.ts`) que o registro precisa. `input_schema` é
/// o JSON Schema cru — o mesmo formato que `toJsonSchema(zodSchema)` produz no lado TS e que o LLM
/// consome direto (`tool-access.ts::mapToolsByPlugin`), não o subconjunto achatado para a UI de
/// settings (`ToolCatalogInputSchema`).
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub plugin_id: String,
    pub requires_window: bool,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: Arc<dyn ToolHandler>,
}

/// Forma de `mapToolsByPlugin` (`tool-access.ts`) — o que `mika:tools-list` devolveria SE esse
/// canal já estivesse reivindicado. `destructive` espelha o campo homônimo de `ToolDef` — só para
/// exibição/observabilidade; quem decide gatear é `execute()`, não a UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
    pub plugin_id: String,
    pub requires_window: bool,
    pub destructive: bool,
}

/// Port de `ToolRegistryService`. Um por processo (ver `registry()`), nunca um por módulo — dois
/// registros seriam dois catálogos divergentes, mesma classe de bug que `ConfigStore` já pagou.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, RegisteredTool>>,
    /// Nomes (já normalizados) que passam pelo gate de aprovação antes de despachar. Não é campo de
    /// `ToolDefinition` de propósito: adicionar campo obrigatório ali quebraria toda construção por
    /// struct literal já existente em módulos fora deste diretório (`vision`, `gamer`, `features`) —
    /// nenhum deles está no meu escopo para editar nesta rodada. Um conjunto à parte, populado só
    /// por [`Self::register_destructive`], evita a mudança sem tocar em nada alheio.
    destructive: RwLock<HashSet<String>>,
    /// Nomes (já normalizados) registrados via [`Self::register_stub`] e ainda não substituídos por
    /// um [`Self::register`] final. `mood`/`desktop`/`person_catalog` cadastram um handler
    /// config-livre na primeira passada de boot (`features::modules()`, sem `AppHandle`) e a versão
    /// completa na segunda (`modules::register_config_bound_tools`, com `AppHandle`/`ConfigStore`) —
    /// ver os cabeçalhos de `features/mood/mod.rs`, `features/desktop/tool.rs` e
    /// `features/person_catalog/tool.rs`. Esse par é uma SUBSTITUIÇÃO PRETENDIDA, não uma colisão:
    /// antes deste campo, [`Self::register`] não distinguia os dois casos e logava
    /// `tool name collision: … (overwriting)` para as três tools em todo boot — ruído que escondia
    /// uma colisão de verdade (dois módulos distintos brigando pelo mesmo nome) no mesmo log.
    stub_names: RwLock<HashSet<String>>,
    /// Nomes que colidiram SEM vir de [`Self::register_stub`] — a classe de defeito real que este
    /// campo existe para tornar testável (ver `two_unrelated_registrations_of_the_same_name_is_an_unexpected_collision`
    /// e `register_stub_colliding_with_a_real_tool_is_also_an_unexpected_collision` em `tests.rs`).
    /// `register_stub`→`register` no mesmo nome nunca entra aqui.
    unexpected_collisions: RwLock<Vec<String>>,
    approval_gate: ApprovalGateService,
    /// Política do usuário (`brain.approveRiskyTools`/`autoApproveTools`) para o gate do LLM — ver
    /// `broker.rs`. `None` até o boot ligar ([`Self::bind_config`]): o gate então PERGUNTA.
    config: RwLock<Option<Arc<crate::config::store::ConfigStore>>>,
    /// Chaves (`access::approval_grant_key`) que o usuário liberou com "Permitir nesta sessão".
    /// Só em memória, de propósito: gravar em `brain.autoApproveTools` (persistido) tornava o
    /// "só desta sessão" permanente. Só `ApprovalScope::Always` vai para o store.
    session_grants: RwLock<HashSet<String>>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            destructive: RwLock::new(HashSet::new()),
            stub_names: RwLock::new(HashSet::new()),
            unexpected_collisions: RwLock::new(Vec::new()),
            approval_gate: ApprovalGateService::new(),
            config: RwLock::new(None),
            session_grants: RwLock::new(HashSet::new()),
        }
    }

    /// Mesma normalização de `normalizeToolName` (`tool-registry.ts`): fora de `[a-zA-Z0-9_]` vira
    /// `_`, underscores nas pontas somem, nome vazio (ou só underscores) cai para `"tool"`.
    pub fn normalize_name(name: &str) -> String {
        let replaced: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        let trimmed = replaced.trim_matches('_');
        if trimmed.is_empty() { "tool".to_string() } else { trimmed.to_string() }
    }

    /// Registra ou SUBSTITUI: colisão de nome normalizado vira `log::warn!`, nunca pânico — mesma
    /// política de `tool-registry.ts` (o último a registrar ganha). Exceção: substituir um handler
    /// que entrou via [`Self::register_stub`] (mesmo nome) é a segunda passada de boot fazendo o
    /// upgrade PRETENDIDO — não conta como colisão inesperada (ver [`Self::stub_names`]).
    pub fn register(&self, mut definition: ToolDefinition, handler: Arc<dyn ToolHandler>) {
        definition.name = Self::normalize_name(&definition.name);
        let mut tools = self.tools.write().unwrap_or_else(|e| e.into_inner());
        if tools.contains_key(&definition.name) {
            let was_stub = self.stub_names.write().unwrap_or_else(|e| e.into_inner()).remove(&definition.name);
            if was_stub {
                log::debug!("tool '{}' upgraded from config-free stub to full implementation", definition.name);
            } else {
                log::warn!("tool name collision: {} (overwriting)", definition.name);
                self.unexpected_collisions
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(definition.name.clone());
                // Primeira passada de boot (`modules::all()`) não retorna `Result` — sem isto, uma
                // colisão real de nome só existia em `unexpected_collisions()`, invisível a
                // `mika:get-health` (ver `feature_health.rs`).
                crate::feature_health::record_registration_failure(
                    "tool_registry",
                    &format!("colisão de nome inesperada: {}", definition.name),
                );
            }
        }
        tools.insert(definition.name.clone(), RegisteredTool { definition, handler });
    }

    /// Registra um handler-tampão da PRIMEIRA passada de boot (sem `AppHandle`/`ConfigStore`) que a
    /// segunda passada substitui por um [`Self::register`] no mesmo nome — o par
    /// `mood`/`desktop`/`person_catalog` documentado em [`Self::stub_names`]. Marca o nome como
    /// "substituição esperada" para que aquele `register` não seja contado como colisão.
    pub fn register_stub(&self, mut definition: ToolDefinition, handler: Arc<dyn ToolHandler>) {
        definition.name = Self::normalize_name(&definition.name);
        let mut tools = self.tools.write().unwrap_or_else(|e| e.into_inner());
        if tools.contains_key(&definition.name) {
            // Um stub nunca deveria colidir com nada — se colidir, é um nome já usado por outra
            // tool de verdade, e ISSO é colisão inesperada de verdade.
            log::warn!("tool name collision: {} (overwriting, from register_stub)", definition.name);
            self.unexpected_collisions.write().unwrap_or_else(|e| e.into_inner()).push(definition.name.clone());
            crate::feature_health::record_registration_failure(
                "tool_registry",
                &format!("colisão de nome inesperada (register_stub): {}", definition.name),
            );
        }
        self.stub_names.write().unwrap_or_else(|e| e.into_inner()).insert(definition.name.clone());
        tools.insert(definition.name.clone(), RegisteredTool { definition, handler });
    }

    pub fn get(&self, name: &str) -> Option<ToolDefinition> {
        let tools = self.tools.read().unwrap_or_else(|e| e.into_inner());
        tools.get(&Self::normalize_name(name)).map(|t| t.definition.clone())
    }

    /// Mesmo registro de [`Self::register`], mas marca a tool como `destructive`: toda chamada
    /// passa pelo gate de aprovação em [`Self::execute`] antes de despachar. Ver o cabeçalho do
    /// arquivo (§"Gate de aprovação"). Chamado em produção por `mcp/register.rs::register_tools` —
    /// toda tool de servidor MCP de terceiro passa por aqui, sem exceção.
    pub fn register_destructive(&self, mut definition: ToolDefinition, handler: Arc<dyn ToolHandler>) {
        definition.name = Self::normalize_name(&definition.name);
        let name = definition.name.clone();
        self.register(definition, handler);
        self.destructive.write().unwrap_or_else(|e| e.into_inner()).insert(name);
    }

    /// Tira uma tool do catálogo (e a marca `destructive` dela). Chamado por `mcp/register.rs`
    /// quando um servidor MCP para, é removido ou reinicia — as tools dele apontariam para um
    /// processo morto. Devolve se havia algo para tirar.
    pub fn unregister(&self, name: &str) -> bool {
        let clean_name = Self::normalize_name(name);
        self.destructive.write().unwrap_or_else(|e| e.into_inner()).remove(&clean_name);
        self.stub_names.write().unwrap_or_else(|e| e.into_inner()).remove(&clean_name);
        self.tools.write().unwrap_or_else(|e| e.into_inner()).remove(&clean_name).is_some()
    }

    fn is_destructive(&self, normalized_name: &str) -> bool {
        self.destructive.read().unwrap_or_else(|e| e.into_inner()).contains(normalized_name)
    }

    /// A MESMA pergunta que [`Self::execute`] faz, para quem precisa decidir ANTES de chamar.
    /// `native.rs` usa isto para não pedir aprovação duas vezes pela mesma chamada: `execute()` já
    /// gateia a tool destrutiva, e o gate de alto risco por AÇÃO (`mika:tool-execute`) só entra
    /// quando este responde `false` — mesma condição do `toolDef?.destructive` em
    /// `ensureBrokerApproval` (`ipc/tool-approval.ts`). Perguntar duas vezes não é só ruído: são
    /// dois diálogos para uma decisão só, e o segundo chega depois de o primeiro já ter liberado.
    pub fn is_destructive_tool(&self, name: &str) -> bool {
        self.is_destructive(&Self::normalize_name(name))
    }

    /// Pede aprovação sem despachar nada — o caminho de `ensureBrokerApproval`, que gateia por
    /// RISCO DA CHAMADA (nome + argumentos) e não pela marca `destructive` da tool.
    pub async fn request_approval(&self, request: ApprovalRequest) -> super::approval::ApprovalDecision {
        self.approval_gate.request_approval(request).await
    }

    /// Instala o handler real do gate de aprovação (janela + IPC — fora deste diretório). Sem
    /// chamador em produção hoje: quem tiver `AppHandle` primeiro (mesmo bloqueio documentado em
    /// `vision::register`) monta um [`ApprovalHandler`] e chama isto uma vez no boot.
    pub fn set_approval_handler(&self, handler: Arc<dyn ApprovalHandler>) {
        self.approval_gate.set_handler(handler);
    }

    pub fn get_all(&self) -> Vec<ToolCatalogItem> {
        let tools = self.tools.read().unwrap_or_else(|e| e.into_inner());
        let destructive = self.destructive.read().unwrap_or_else(|e| e.into_inner());
        tools
            .values()
            .map(|t| ToolCatalogItem {
                name: t.definition.name.clone(),
                description: t.definition.description.clone(),
                input_schema: t.definition.input_schema.clone(),
                plugin_id: t.definition.plugin_id.clone(),
                requires_window: t.definition.requires_window,
                destructive: destructive.contains(&t.definition.name),
            })
            .collect()
    }

    /// Valida `args` contra o schema declarado ANTES de despachar (equivalente a `parseToolInput`
    /// / Zod `safeParse` do lado TS), depois — se a tool for `destructive` — pede aprovação antes de
    /// chamar o handler. Tool desconhecida e argumento fora do schema voltam como `Err` nomeado;
    /// aprovação negada volta como resultado de negócio (`{ success: false, error: "user_denied" }`,
    /// mesma forma de `ToolRegistryService.execute` no TS), nunca como exceção. Nunca panic, nunca
    /// handler chamado com entrada ruim ou sem aprovação.
    pub async fn execute(&self, name: &str, args: Value) -> Result<Value, String> {
        self.execute_as(ToolOrigin::System, name, args).await
    }

    /// [`Self::execute`] com a ORIGEM da chamada. `Llm` (o loop de chat) passa por
    /// `is_llm_gated_call` — ver `broker.rs` — antes de despachar; sem aprovação o handler nunca roda.
    pub async fn execute_as(&self, origin: ToolOrigin, name: &str, args: Value) -> Result<Value, String> {
        let clean_name = Self::normalize_name(name);
        let (schema, handler) = {
            let tools = self.tools.read().unwrap_or_else(|e| e.into_inner());
            let Some(entry) = tools.get(&clean_name) else {
                let mut known: Vec<&str> = tools.keys().map(String::as_str).collect();
                known.sort_unstable();
                return Err(format!("Tool not found: {clean_name}. Available tools: {}", known.join(", ")));
            };
            (entry.definition.input_schema.clone(), entry.handler.clone())
        };

        let args = coerce_args(&schema, args);
        validate_args(&schema, &args)?;

        if let Some(denial) = self.gate_llm_call(origin, &clean_name, &args).await {
            return Ok(denial);
        }

        if self.is_destructive(&clean_name) {
            let tool_call_id = format!("tool-{clean_name}-{}", uuid::Uuid::new_v4());
            let decision = self
                .approval_gate
                .request_approval(ApprovalRequest { tool_call_id, tool_name: clean_name.clone(), args: args.clone() })
                .await;
            if !decision.approved {
                log::warn!("tool execution denied by approval gate: {clean_name}");
                return Ok(json!({ "success": false, "error": "user_denied" }));
            }
        }

        // O "nunca panic" acima inclui o handler: um panic numa tool vira `Err` (o modelo recebe a
        // falha e o turno continua) em vez de derrubar a task do turno e deixar o HUD "pensando".
        match AssertUnwindSafe(handler.call(args)).catch_unwind().await {
            Ok(outcome) => outcome,
            Err(payload) => {
                let message = panic_message(payload.as_ref());
                log::error!("tool {clean_name} deu panic: {message}");
                Err(format!("tool panicked: {message}"))
            }
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "sem mensagem".to_string())
}

/// Registro único por processo — mesmo padrão de `OnceLock` já usado em `llm.rs`/`telemetry.rs`
/// para estado que não tem por que viver dentro do `ShellState` do Tauri (ver `state.rs`, fora do
/// escopo deste arquivo). Ponto de extensão: um módulo já portado chama
/// `crate::modules::tools::registry().register(...)` no próprio setup; nada aqui precisa mudar
/// para um módulo novo se cadastrar.
static REGISTRY: OnceLock<ToolRegistry> = OnceLock::new();

pub fn registry() -> &'static ToolRegistry {
    REGISTRY.get_or_init(ToolRegistry::new)
}

// Suíte em arquivo separado só pela regra de 300 linhas do AGENTS.md (`mod.rs` deste mesmo
// diretório já documentou o mesmo motivo para `tools` virar diretório) — continua sendo o mesmo
// módulo `registry::tests`, com o mesmo acesso a privado que teria inline.
#[cfg(test)]
impl ToolRegistry {
    /// Nomes que colidiram fora do par stub→real esperado — a condição que os testes de colisão
    /// (`tests.rs`) transformam em falha de teste em vez de só uma linha de log que ninguém lê.
    pub fn unexpected_collisions(&self) -> Vec<String> {
        self.unexpected_collisions.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// **check:tool-coverage (Rust; o script TS foi aposentado em 2026-09-23).** O script TS media 22 tools no
/// catálogo do LLM até a migração para Tauri esvaziar o TS: caiu para 3 (mood/scheduler/system-info,
/// as únicas que sobraram lá) e o cabeçalho do script já confessava o buraco — "a cobertura Rust NÃO
/// tem gate equivalente ainda". As outras 19 tools portadas para `modules/tools/**` e
/// `modules/features/*/tool.rs` ficaram sem ESTE gate: uma tool que só o modelo chama, e cujo nome
/// nenhum teste escreve, quebra em silêncio — ninguém clica nela para notar.
///
/// Medido em 2026-09-22: 20 tools com `ToolDefinition { name: "...".into(), … }` em código de
/// PRODUÇÃO (fora de `#[cfg(test)] mod tests { … }`) sob `src-tauri/src/modules/**`. Das 20, só
/// `reminder` (`features/scheduler/tool.rs`) não tinha a própria string em teste nenhum — os testes
/// construíam `ReminderToolHandler` direto, sem nunca escrever `"reminder"`. Fechado nesta rodada com
/// `tool_definition_name_is_reminder` (`features/scheduler/tool.rs::tests`); o piso abaixo é 20/20,
/// não decoração.
#[cfg(test)]
mod tool_coverage_gate {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use regex::Regex;

    /// `ToolDefinition.name` é sempre `name: "snake_case".into()` — distinto de
    /// `Namespace { name: "…", … }` (schema de config, sem `.into()`) e de `tool_name: "…".into()`
    /// (`ApprovalRequest`, ex.: o probe `e2e_hooks_probe`), por isso o lookbehind negativo.
    fn tool_names_in(source: &str) -> Vec<String> {
        let re = Regex::new(r#"(?:^|[^A-Za-z0-9_])name:\s*"([a-z][a-z0-9_]*)"\s*\.into\(\)"#).expect("regex válida");
        re.captures_iter(source).map(|cap| cap[1].to_string()).collect()
    }

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// Corta o conteúdo do arquivo no primeiro `mod tests {` — o mesmo corte que
    /// `modules/mod.rs::nothing_the_boot_calls_uses_bare_tokio_spawn` já usa para separar produção
    /// de teste inline.
    fn production_only(source: &str) -> &str {
        source.split("mod tests {").next().unwrap_or(source)
    }

    /// Ferramentas declaradas em código de produção sob `src-tauri/src/modules/**`, mapeadas ao
    /// arquivo que as declara (o primeiro achado vence — mesma política do gate TS).
    fn declared_tools(modules_root: &Path) -> BTreeMap<String, PathBuf> {
        let mut files = Vec::new();
        rs_files(modules_root, &mut files);
        files.sort();

        let mut declared = BTreeMap::new();
        for file in files {
            let name_matches_test_file = file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_tests.rs") || n == "tests.rs");
            if name_matches_test_file {
                continue;
            }
            let source = fs::read_to_string(&file).unwrap_or_default();
            for name in tool_names_in(production_only(&source)) {
                declared.entry(name).or_insert_with(|| file.clone());
            }
        }
        declared
    }

    /// Todo texto de teste ao alcance do repo: as suítes Rust inline (`mod tests { … }`), os
    /// arquivos `*_tests.rs`/`tests.rs`, e os specs Playwright/homologação em `apps/desktop/e2e`
    /// (que já sobem o binário Tauri — ver `AGENTS.md`). Uma tool só chamada pelo modelo, e cuja
    /// string de nome não aparece em nenhum dos dois, não tem prova nenhuma de que funciona.
    fn test_corpus(src_root: &Path, repo_root: &Path) -> String {
        let mut corpus = String::new();

        let mut files = Vec::new();
        rs_files(src_root, &mut files);
        for file in files {
            let source = fs::read_to_string(&file).unwrap_or_default();
            let is_test_file = file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_tests.rs") || n == "tests.rs");
            if is_test_file {
                corpus.push_str(&source);
            } else if let Some(idx) = source.find("mod tests {") {
                corpus.push_str(&source[idx..]);
            }
        }

        let e2e_dir = repo_root.join("apps").join("desktop").join("e2e");
        let mut e2e_files = Vec::new();
        fn ts_files(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    ts_files(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("ts") {
                    out.push(path);
                }
            }
        }
        ts_files(&e2e_dir, &mut e2e_files);
        for file in e2e_files {
            corpus.push_str(&fs::read_to_string(&file).unwrap_or_default());
        }

        corpus
    }

    #[test]
    fn every_registered_tool_name_is_referenced_by_some_test() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .canonicalize()
            .expect("raiz do repo");

        let declared = declared_tools(&src_root.join("modules"));

        // Piso de alcance: uma extração quebrada devolveria zero/poucas tools e o gate passaria por
        // vacuidade — a mesma classe de falso-verde que `modules::mod.rs` já documenta noutro teste
        // desta suíte. Medido em 2026-09-22: 20 tools de produção.
        assert!(
            declared.len() >= 20,
            "só {} tool(s) reconhecida(s) sob modules/** ({:?}) — o extrator (`tool_names_in`) parou \
             de casar com o código; não confie neste gate até corrigi-lo",
            declared.len(),
            declared.keys().collect::<Vec<_>>()
        );

        let corpus = test_corpus(&src_root, &repo_root);
        let uncovered: Vec<String> = declared
            .iter()
            .filter(|(name, _)| !corpus.contains(&format!("\"{name}\"")))
            .map(|(name, file)| format!("{name} ({})", file.display()))
            .collect();

        assert!(
            uncovered.is_empty(),
            "tool(s) registrada(s) em produção sem NENHUM teste (Rust ou e2e) escrevendo o próprio \
             nome — só o modelo as exercita: {uncovered:?}"
        );
    }
}
