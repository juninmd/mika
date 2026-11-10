//! Porte de `main/ipc/tool-access.ts` + `services/brain/tool-loop/tool-risk.ts` — quem pode pedir
//! o quê, e o que é "chamada de alto risco".
//!
//! Sai de `native.rs` porque é a metade PURA: nenhuma função aqui toca `AppHandle`, janela ou
//! registro — recebe contexto e nome, devolve veredito. É o que torna a política testável sem um
//! app Tauri, e é onde o teste de contrato lê o fonte TS para provar que as duas listas não
//! divergiram.
//!
//! **Negar por padrão em todo ramo.** Nome desconhecido, janela desconhecida, `pluginId` ausente:
//! tudo isso é recusa, nunca "deixa passar porque não sei". O TS já é assim e a inversão aqui seria
//! silenciosa — a tool executaria, e o único sinal seria o efeito colateral no mundo.

use serde_json::Value;

use super::registry::ToolDefinition;
use crate::protocol::WindowType;

/// Espelha `TOOL_EXECUTE_ALLOWED_WINDOW_TYPES` (`tool-access.ts`). `Wizard` fica de FORA: o
/// onboarding não executa tool nenhuma, e incluí-lo abriria superfície durante a única tela que
/// roda antes do usuário ter configurado qualquer política.
fn is_allowed_caller(window: WindowType) -> bool {
    matches!(window, WindowType::Main | WindowType::Settings | WindowType::PluginView)
}

/// Espelha `HIGH_RISK_TOOL_NAMES` e `HIGH_RISK_TOOL_PREFIXES` (`tool-risk.ts`).
const HIGH_RISK_NAMES: &[&str] = &["clear_history"];
const HIGH_RISK_PREFIXES: &[&str] = &["mcp_"];

/// `getToolRiskTier(name) === 'high'`. O tier `medium` (`vision_capture`/`vision_read`) não tem
/// consumidor nesta política — ele só existe no TS para o loop de tools decidir outra coisa —,
/// então aqui a pergunta é binária de propósito, em vez de um enum de três variantes sem terceiro
/// leitor.
fn is_high_risk_tier(clean_name: &str) -> bool {
    HIGH_RISK_NAMES.contains(&clean_name) || HIGH_RISK_PREFIXES.iter().any(|p| clean_name.starts_with(p))
}

/// Espelha `DESTRUCTIVE_CALL_BY_TOOL` (`tool-risk.ts`): o risco de algumas tools depende da AÇÃO
/// pedida, não do nome. `desktop` é o caso invertido — tudo é destrutivo MENOS `window`/`process`,
/// porque `launch` executa binário e um default "só o que eu listei é perigoso" deixaria passar
/// qualquer ação nova que alguém acrescente à tool depois.
fn action_scoped_risk(clean_name: &str, args: &Value) -> Option<bool> {
    let action = args.get("action").and_then(Value::as_str);
    match clean_name {
        "person_catalog" => Some(matches!(action, Some("delete" | "delete_fact" | "delete_relationship"))),
        "user_profile" => Some(action == Some("forget")),
        "memory_console" => Some(matches!(action, Some("clear_episodic" | "delete_episodic"))),
        "desktop" => Some(!matches!(action, Some("window" | "process"))),
        "steam" => Some(action == Some("library") && args.get("mode").and_then(Value::as_str) == Some("open")),
        _ => None,
    }
}

/// Porte de `isHighRiskCall`. Tool com risco por ação responde pela AÇÃO; as demais, pelo tier.
pub fn is_high_risk_call(name: &str, args: &Value) -> bool {
    let clean_name = super::registry::ToolRegistry::normalize_name(name);
    match action_scoped_risk(&clean_name, args) {
        Some(risky) => risky,
        None => is_high_risk_tier(&clean_name),
    }
}

/// Acima disto, `query`/`#fragmento`/segmento de caminho de uma URL aberta pelo MODELO deixa de
/// ser "uma pesquisa/rota" e passa a poder carregar memória/perfil/clipboard para fora.
const EXFIL_URL_PART_MAX: usize = 64;
/// Caminho inteiro (um sha1 de commit ou um slug de artigo cabem folgados; base64 de um trecho não).
const EXFIL_PATH_MAX: usize = 128;
/// Rótulo de subdomínio (`<hex>.evil.example`): o DNS entrega o rótulo ao dono do domínio sem
/// nenhuma requisição HTTP. Um hash de 20 bytes em hex tem 40 chars.
const EXFIL_HOST_LABEL_MAX: usize = 40;
/// Texto de `web search` (`query`) acima disto é um trecho colado, não uma pergunta.
const EXFIL_SEARCH_QUERY_MAX: usize = 200;

/// `web fetch` (`url`) e `desktop launch` (`target`) com URL cuja query/fragmento/caminho/subdomínio
/// passa dos tetos acima, com credencial embutida (`user:senha@`) ou que nem parseia; e `web search`
/// com `query` longa. Não reconhece TRECHO de memória/perfil/clipboard dentro da URL nem rastreia
/// se a URL veio de uma página/tela/tool em vez do usuário (exigiria ledger de origem por turno): a
/// regra de tamanho fecha o canal de carga útil, e `desktop launch` pede aprovação sempre.
pub fn is_exfil_suspect_call(name: &str, args: &Value) -> bool {
    let clean_name = super::registry::ToolRegistry::normalize_name(name);
    let action = args.get("action").and_then(Value::as_str);
    let (key, search) = match (clean_name.as_str(), action) {
        ("web", Some("fetch")) => ("url", false),
        ("desktop", Some("launch")) => ("target", false),
        ("web", Some("search")) => ("query", true),
        _ => return false,
    };
    let Some(raw) = args.get(key).and_then(Value::as_str).map(str::trim) else { return false };
    if search {
        return raw.chars().count() > EXFIL_SEARCH_QUERY_MAX;
    }
    if !raw.get(..4).is_some_and(|scheme| scheme.eq_ignore_ascii_case("http")) {
        return false;
    }
    url::Url::parse(raw).map_or(true, |url| url_carries_payload(&url))
}

fn url_carries_payload(url: &url::Url) -> bool {
    let long_host_label = url.host_str().is_some_and(|host| host.split('.').any(|label| label.len() > EXFIL_HOST_LABEL_MAX));
    let long_path = url.path().len() > EXFIL_PATH_MAX
        || url.path_segments().is_some_and(|mut segments| segments.any(|segment| segment.len() > EXFIL_URL_PART_MAX));
    long_host_label
        || long_path
        || url.query().is_some_and(|q| q.len() > EXFIL_URL_PART_MAX)
        || url.fragment().is_some_and(|f| f.len() > EXFIL_URL_PART_MAX)
        || !url.username().is_empty()
        || url.password().is_some()
}

/// O que SÓ o LLM pede sob aprovação, somado a [`is_high_risk_call`]: alterar o perfil do usuário,
/// despejar dados pessoais (`export_data`, `list_memories`), apagar hábitos, ligar o microfone e URL
/// com carga útil. A UI do usuário
/// (IPC) não passa por aqui — o clique dele já é a ação explícita.
pub fn is_llm_gated_call(name: &str, args: &Value) -> bool {
    if is_high_risk_call(name, args) {
        return true;
    }
    let clean_name = super::registry::ToolRegistry::normalize_name(name);
    let action = args.get("action").and_then(Value::as_str);
    match clean_name.as_str() {
        "user_profile" => matches!(action, Some("set" | "learn" | "set_personality")),
        "export_data" | "list_memories" => true,
        // Apagar o contador de hábitos e ligar o microfone contínuo: ação do usuário, não do modelo.
        "habits" => action == Some("clear"),
        // O mesmo alcance de `person_catalog delete`/`user_profile set|forget` por outra porta: "esquecer"
        // uma pessoa em todo lugar, apagar fato/relação, trocar ou apagar o nome. Só o clique do usuário
        // (IPC) não passa por aqui — `is_high_risk_call` não muda.
        "memory_console" => {
            matches!(action, Some("forget_person" | "delete_fact" | "delete_relationship" | "forget_user_name" | "set_user_name" | "update_fact"))
        }
        "voice" if action == Some("listen_start") => true,
        _ => is_exfil_suspect_call(&clean_name, args),
    }
}

/// Porte de `approvalGrantKey` (`approval-wrap.ts`): a chave que `brain.autoApproveTools` guarda.
/// Tool com risco por ação grava `nome:acao`, para que aprovar `desktop:window` uma vez não
/// aprove `desktop:launch` para sempre — a diferença inteira entre as duas.
pub fn approval_grant_key(name: &str, args: &Value) -> String {
    let clean_name = super::registry::ToolRegistry::normalize_name(name);
    if action_scoped_risk(&clean_name, &Value::Null).is_none() {
        return clean_name;
    }
    match args.get("action").and_then(Value::as_str) {
        Some(action) => format!("{clean_name}:{action}"),
        None => clean_name,
    }
}

/// Porte de `assertToolListingAllowed`. Só a superfície; o catálogo em si não é filtrado por
/// chamador no TS, e inventar um filtro aqui divergiria do que a UI de settings já espera ver.
pub fn assert_tool_listing_allowed(window: WindowType) -> Result<(), String> {
    if is_allowed_caller(window) {
        return Ok(());
    }
    Err("Tool listing denied: caller window is not allowed".into())
}

/// Porte de `assertToolExecutionAllowed`. Três recusas, nesta ordem — a mesma do TS, que importa:
/// a checagem de janela vem antes da de identidade de plugin, e a de política por tier vem por
/// último, para que uma `plugin-view` sem identidade seja recusada por ISSO, não por um tier que
/// ela nem chegaria a consultar.
pub fn assert_tool_execution_allowed(
    window: WindowType,
    plugin_id: Option<&str>,
    tool_name: &str,
    definition: Option<&ToolDefinition>,
) -> Result<(), String> {
    if !is_allowed_caller(window) {
        return Err("Tool execution denied: caller window is not allowed".into());
    }

    if window == WindowType::PluginView {
        let Some(caller) = plugin_id.filter(|id| !id.is_empty()) else {
            return Err("Tool execution denied: missing plugin identity".into());
        };
        // Nome desconhecido também é negado: sem definição não há como AFIRMAR que a tool é do
        // plugin que pediu, e "não sei" nunca vira "pode".
        match definition {
            Some(def) if def.plugin_id == caller => {}
            _ => return Err("Tool execution denied: plugin can only execute its own tools".into()),
        }
    }

    if window != WindowType::Main && is_high_risk_tier(&super::registry::ToolRegistry::normalize_name(tool_name)) {
        return Err(format!("Tool execution denied by policy: {tool_name}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // `tool-risk.ts`/`tool-access.ts` foram removidos (backend Node deletado na migração para
    // Tauri) — os denominadores de segurança que eles fixavam viram snapshots congelados aqui,
    // extraídos do fonte no momento da deleção. `action_scoped_risk`/`is_high_risk_tier`/
    // `is_allowed_caller` continuam sendo a única fonte de verdade em produção; os snapshots só
    // garantem que uma entrada removida sem querer ainda reprova, agora contra um literal em vez
    // de `include_str!`.
    const FROZEN_DESTRUCTIVE_CALL_TOOLS: &[&str] =
        &["person_catalog", "user_profile", "memory_console", "desktop", "steam"];
    const FROZEN_HIGH_RISK_NAMES: &[&str] = &["clear_history"];

    /// O denominador desta política era mantido no TS (`DESTRUCTIVE_CALL_BY_TOOL`). Uma tool
    /// removida sem querer do snapshot congelado — ou de `action_scoped_risk` — é uma chamada
    /// perigosa que deixa de pedir aprovação, sem erro de compilação, sem teste vermelho, sem
    /// nada na tela.
    #[test]
    fn every_frozen_action_scoped_tool_has_a_branch_here() {
        for name in FROZEN_DESTRUCTIVE_CALL_TOOLS {
            assert!(
                action_scoped_risk(name, &Value::Null).is_some(),
                "'{name}' tinha risco por ação antes da deleção do TS e nenhum ramo em action_scoped_risk — \
                 a chamada perigosa passaria sem pedir aprovação"
            );
        }
    }

    /// A outra metade: o nome de alto risco por TIER. Hoje é um só, e é o mais destrutivo do
    /// catálogo inteiro.
    #[test]
    fn the_high_risk_names_match_the_frozen_set() {
        assert_eq!(HIGH_RISK_NAMES, FROZEN_HIGH_RISK_NAMES);
        assert!(is_high_risk_tier("mcp_anything"), "toda tool de servidor MCP é alto risco por prefixo");
        assert!(!is_high_risk_tier("mood"), "tool comum não pode virar alto risco");
    }

    /// As três superfícies permitidas, congeladas do fonte TS antes da deleção — e a prova de que
    /// as OUTRAS são negadas.
    #[test]
    fn the_allowed_caller_surfaces_match_the_frozen_list() {
        for entry in ["main", "settings", "plugin-view"] {
            assert!(is_allowed_caller(WindowType::from_label(entry)), "'{entry}' era permitido antes da deleção do TS e está negado aqui");
        }

        assert!(!is_allowed_caller(WindowType::Wizard), "o wizard não executa tool — abrir isso é superfície de graça");
    }

    #[test]
    fn desktop_is_risky_for_everything_except_window_and_process() {
        assert!(is_high_risk_call("desktop", &json!({ "action": "launch" })));
        assert!(!is_high_risk_call("desktop", &json!({ "action": "window" })));
        assert!(!is_high_risk_call("desktop", &json!({ "action": "process" })));
        // Ação NOVA, que ninguém listou: perigosa por padrão. É o ponto do `exceptAction`.
        assert!(is_high_risk_call("desktop", &json!({ "action": "whatever_comes_next" })));
        assert!(is_high_risk_call("desktop", &json!({})));
    }

    #[test]
    fn steam_is_only_risky_when_it_opens_a_game() {
        assert!(is_high_risk_call("steam", &json!({ "action": "library", "mode": "open" })));
        assert!(!is_high_risk_call("steam", &json!({ "action": "library", "mode": "list" })));
        assert!(!is_high_risk_call("steam", &json!({ "action": "achievements" })));
    }

    /// A chave de concessão é o que separa "aprovei ver a janela" de "aprovei executar binário".
    /// Sem o sufixo de ação, um `scope: 'always'` em `desktop:window` liberaria `desktop:launch`
    /// para sempre — escalada de privilégio por reuso de chave.
    #[test]
    fn the_grant_key_carries_the_action_for_action_scoped_tools() {
        assert_eq!(approval_grant_key("desktop", &json!({ "action": "launch" })), "desktop:launch");
        assert_eq!(approval_grant_key("desktop", &json!({ "action": "window" })), "desktop:window");
        assert_eq!(approval_grant_key("clear_history", &json!({})), "clear_history");
        // Sem `action` legível, cai para o nome — nunca para uma chave inventada.
        assert_eq!(approval_grant_key("desktop", &json!({})), "desktop");
    }

    #[test]
    fn listing_is_denied_for_a_surface_that_is_not_in_the_list() {
        assert!(assert_tool_listing_allowed(WindowType::Main).is_ok());
        assert!(assert_tool_listing_allowed(WindowType::Settings).is_ok());
        assert!(assert_tool_listing_allowed(WindowType::Wizard).is_err());
    }

    fn definition(plugin_id: &str) -> ToolDefinition {
        ToolDefinition {
            name: "some_tool".into(),
            description: None,
            input_schema: json!({ "type": "object" }),
            plugin_id: plugin_id.into(),
            requires_window: false,
        }
    }

    #[test]
    fn a_plugin_view_can_only_execute_its_own_tools() {
        let own = definition("mika.gamer");
        assert!(
            assert_tool_execution_allowed(WindowType::PluginView, Some("mika.gamer"), "some_tool", Some(&own)).is_ok()
        );

        let other = definition("mika.brain");
        assert!(
            assert_tool_execution_allowed(WindowType::PluginView, Some("mika.gamer"), "some_tool", Some(&other))
                .is_err(),
            "plugin-view executando tool de OUTRO plugin é a escalada que esta guarda existe para barrar"
        );
    }

    /// Nome que o registro não conhece é recusa, não "passa e falha depois": sem definição não há
    /// como afirmar a quem a tool pertence.
    #[test]
    fn a_plugin_view_is_denied_for_an_unknown_tool_name() {
        assert!(
            assert_tool_execution_allowed(WindowType::PluginView, Some("mika.gamer"), "nope", None).is_err()
        );
    }

    #[test]
    fn a_plugin_view_without_identity_is_denied() {
        let own = definition("mika.gamer");
        assert!(assert_tool_execution_allowed(WindowType::PluginView, None, "some_tool", Some(&own)).is_err());
        assert!(assert_tool_execution_allowed(WindowType::PluginView, Some(""), "some_tool", Some(&own)).is_err());
    }

    /// `clear_history` é alto risco por tier: só a janela principal executa. Settings pede pela
    /// tela de privacidade, que é outro caminho — não esta tool.
    #[test]
    fn high_tier_tools_are_main_window_only() {
        assert!(assert_tool_execution_allowed(WindowType::Main, None, "clear_history", None).is_ok());
        assert!(assert_tool_execution_allowed(WindowType::Settings, None, "clear_history", None).is_err());
        assert!(assert_tool_execution_allowed(WindowType::Settings, None, "mood", None).is_ok());
    }

    #[test]
    fn an_unlisted_surface_is_denied_before_anything_else_is_consulted() {
        assert!(assert_tool_execution_allowed(WindowType::Wizard, None, "mood", None).is_err());
    }

    /// O que SÓ o LLM precisa pedir: `user_profile` mexendo no perfil, as tools que despejam dados
    /// pessoais, e URL com carga útil. Nada disto entra em `is_high_risk_call` — a UI do usuário
    /// (IPC) já é ação explícita dele e não pode ganhar diálogo novo.
    #[test]
    fn llm_gate_adds_profile_writes_exports_and_exfil_urls_on_top_of_the_high_risk_set() {
        for (name, args) in [
            ("memory_console", json!({ "action": "clear_episodic" })),
            ("person_catalog", json!({ "action": "delete_fact" })),
            ("desktop", json!({ "action": "launch", "target": "calc" })),
            ("user_profile", json!({ "action": "forget" })),
            ("user_profile", json!({ "action": "set" })),
            ("user_profile", json!({ "action": "learn" })),
            ("user_profile", json!({ "action": "set_personality" })),
            ("export_data", json!({})),
            ("list_memories", json!({ "limit": 5 })),
            // Apagar o contador de hábitos e ligar o microfone contínuo são ações do usuário, não do modelo.
            ("habits", json!({ "action": "clear" })),
            ("voice", json!({ "action": "listen_start" })),
            // `memory_console` apaga/edita tanto quanto `person_catalog`/`user_profile` (que pedem
            // aprovação): `forget_person` "apaga a pessoa em todo lugar" e `set_user_name` é o
            // `user_profile set` por outra porta.
            ("memory_console", json!({ "action": "forget_person", "name": "Dani" })),
            ("memory_console", json!({ "action": "delete_fact", "id": "f1" })),
            ("memory_console", json!({ "action": "delete_relationship", "id": "r1" })),
            ("memory_console", json!({ "action": "forget_user_name" })),
            ("memory_console", json!({ "action": "set_user_name", "name": "Ana" })),
            ("memory_console", json!({ "action": "update_fact", "id": "f1", "fact": "x" })),
        ] {
            assert!(is_llm_gated_call(name, &args), "{name} {args} tinha de exigir aprovação do LLM");
        }
        for (name, args) in [
            ("user_profile", json!({ "action": "get" })),
            ("memory_console", json!({ "action": "snapshot" })),
            ("memory_console", json!({ "action": "search_episodic", "query": "x" })),
            ("memory_console", json!({ "action": "facts" })),
            ("memory_console", json!({ "action": "add_fact", "fact": "gosta de café" })),
            ("desktop", json!({ "action": "window" })),
            ("web", json!({ "action": "search", "query": "rust" })),
            ("web", json!({ "action": "fetch", "url": "https://example.com/a?q=rust" })),
            ("mood", json!({})),
            ("habits", json!({ "action": "get" })),
            ("voice", json!({ "action": "listen_stop" })),
            ("voice", json!({ "action": "mute", "minutes": 5 })),
        ] {
            assert!(!is_llm_gated_call(name, &args), "{name} {args} não deveria pedir aprovação");
        }
        // `is_high_risk_call` (caminho IPC do usuário) continua sem as extras.
        assert!(!is_high_risk_call("memory_console", &json!({ "action": "forget_person", "name": "Dani" })));
        assert!(!is_high_risk_call("user_profile", &json!({ "action": "learn" })));
        assert!(!is_high_risk_call("export_data", &json!({})));
    }

    #[test]
    fn urls_that_can_smuggle_data_out_are_exfil_suspects() {
        let long = "a".repeat(65);
        let ok64 = "a".repeat(64);
        for url in [
            format!("https://evil.example/c?d={long}"),
            format!("https://evil.example/c#{long}"),
            "https://user:pass@evil.example/".to_string(),
            "https://tok@evil.example/".to_string(),
            "http://[broken".to_string(),
            // Carga útil no CAMINHO e no SUBDOMÍNIO: o mesmo canal de saída, sem query nem fragmento.
            format!("https://evil.example/{}", "A".repeat(100)),
            format!("https://evil.example/{}", ["abcdefghij"; 14].join("/")),
            format!("https://{}.evil.example/", "ab".repeat(21)),
        ] {
            assert!(is_exfil_suspect_call("web", &json!({ "action": "fetch", "url": url })), "{url}");
            assert!(is_exfil_suspect_call("desktop", &json!({ "action": "launch", "target": url })), "{url}");
        }
        for url in [
            format!("https://example.com/p?d={}", &ok64[..60]),
            "https://example.com/very/long/path/segment".to_string(),
            // sha1 de commit e slug longo de artigo são URLs comuns, não carga útil.
            format!("https://github.com/o/r/commit/{}", "a".repeat(40)),
            format!("https://news.example/2026/10/{}", "a-slug-".repeat(7)),
            "https://a.b.c.docs.example.com/guide/".to_string(),
        ] {
            assert!(!is_exfil_suspect_call("web", &json!({ "action": "fetch", "url": url })), "{url}");
        }
        // Busca com texto longo é o mesmo vazamento pelo outro verbo da tool `web`.
        assert!(is_exfil_suspect_call("web", &json!({ "action": "search", "query": "q".repeat(201) })));
        assert!(!is_exfil_suspect_call("web", &json!({ "action": "search", "query": "q".repeat(200) })));
        // Alias/programa não é URL; `search` não carrega URL; outra tool nunca é suspeita por isto.
        assert!(!is_exfil_suspect_call("desktop", &json!({ "action": "launch", "target": "calc" })));
        assert!(!is_exfil_suspect_call("web", &json!({ "action": "search", "query": "rust", "url": format!("https://x.example/?q={long}") })));
        assert!(!is_exfil_suspect_call("mood", &json!({ "url": format!("https://x.example/?q={long}") })));
    }
}
