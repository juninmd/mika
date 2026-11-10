//! Suíte de `mod.rs`, separada só pela regra de 300 linhas — ver o comentário no ponto de
//! declaração (`super::tests`). Mesmo módulo lógico `tools::tests`; `super::` continua enxergando
//! tudo que `mod.rs` declara, público ou não.

/// Os quatro canais, ou nenhum — nunca um subconjunto.
///
/// Este teste já cobrou o contrário ("nenhum canal reivindicado hoje"), e virou vermelho por
/// ACERTO quando `native.rs` chegou. O que ele guarda agora é a mesma invariante pelo outro lado:
/// `bridge.rs` roteia por canal INTEIRO, então reivindicar `tool-approval-response` sem
/// `tool-execute` (ou vice-versa) interceptaria a resposta de uma aprovação que o outro processo
/// está esperando — a chamada fica pendurada até o backstop. Ver o cabeçalho de `native.rs`.
#[test]
fn claims_all_four_tool_channels_or_none() {
    let modules = super::modules();
    assert_eq!(modules.len(), 1, "o módulo de tools é um só — dois donos derrubariam o boot em NativeRegistry::new");

    let claimed: Vec<&str> = modules[0].channels().to_vec();
    for channel in ["mika:tools-list", "mika:tools-list-by-plugin", "mika:tool-execute", "tool-approval-response"] {
        assert!(claimed.contains(&channel), "canal '{channel}' deixou de ser reivindicado — os quatro migram juntos");
    }
    assert_eq!(claimed.len(), 4, "reivindicou {claimed:?} — nem mais nem menos que os quatro");
}

/// Ancora os 4 canais desta investigação contra o contrato real — se um deles mudar ou sumir,
/// a investigação acima (que os cita de cabeça) fica desatualizada e este teste avisa.
#[test]
fn candidate_channels_exist_in_the_ts_contract() {
    let channels = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
    for channel in ["mika:tools-list", "mika:tools-list-by-plugin", "mika:tool-execute", "tool-approval-response"] {
        assert!(
            channels.contains(&format!("'{channel}'")),
            "canal candidato {channel} não existe (mais) em desktop-ipc-channels.ts — releia a investigação"
        );
    }
}

// `ipc/tool-access.ts` foi removido (backend Node deletado na migração para Tauri) — o
// invariante "`tools-list-by-plugin` filtra o mesmo registro que `tools-list`, não é fonte
// própria" agora é comportamento do próprio `super::registry()` em Rust, sem um segundo lado TS
// para divergir.

/// Prova positiva de que o número de tools config-livres não é citado de memória: chama os mesmos
/// `modules()` que `lib.rs`/`native::registry()` chamam em produção e mede o registro de processo
/// de verdade. As config-vinculadas (`register_config_bound_tools`) cada uma já tem seu próprio
/// teste ao vivo no módulo dono — `brain/tools/mod.rs`, `vision/mod.rs`, `features/*/tool.rs`.
///
/// `content` NÃO está nesta lista apesar do cabeçalho de uma rodada anterior tê-la citado como
/// config-livre: ela fecha sobre `ConfigStore` (persona/idioma do prompt) e só nasce na segunda
/// passada de boot (`register_config_bound_tools`, provado ao vivo por
/// `brain::tools::tests::config_bound_registration_publishes_web_and_content`), não como efeito
/// colateral de `modules()`. Chamando só `modules()` — sem `ConfigStore` — ela nunca aparece aqui;
/// checá-la nesta lista era o defeito, não uma flake de `OnceLock` concorrente.
///
/// **`steam` saiu da lista em 2026-09-22, e o motivo é o mesmo de `content`.** A tool passou a
/// precisar de `ConfigStore` (a chave da API Web, para `action=achievements`) e migrou para
/// `gamer::register_session_tools`, a segunda passada. Ela continuava passando AQUI — mas por
/// contaminação: o registro é um `OnceLock` de processo, e na suíte inteira
/// `gamer::tests::register_session_tools_registers_...` registra `steam` antes deste teste rodar.
/// Verde por vizinho, não por verdade — a mesma armadilha que o comentário de bloco lá embaixo
/// descreve para os pins negativos, agora do lado positivo.
#[test]
fn native_registry_has_the_config_free_tools_measured_this_round() {
    let _ = crate::modules::gamer::modules();
    let _ = crate::modules::features::modules();
    let _ = crate::modules::brain::modules();

    let names: Vec<String> = super::registry().get_all().into_iter().map(|t| t.name).collect();
    for expected in ["track_gamer_mood", "mood", "desktop", "match_effect", "memory_console", "collaboration"] {
        assert!(names.contains(&expected.to_string()), "esperava {expected} no registro nativo, tinha {names:?}");
    }
}

// `embedding.feature.ts`/`browser.feature.ts` foram removidos (backend Node deletado na migração
// para Tauri). O denominador de 24 tools que eles ajudavam a medir era uma contagem de
// investigação da migração — concluída (ver o cabeçalho abaixo); sem twin TS, não sobra o que
// remedir.

// -- O denominador do catálogo, e o que ainda falta dele ------------------------------------------
//
// Estes testes nasceram como pins NEGATIVOS (`registry().get(x).is_none()`) medindo a lacuna: 14 de
// 24 tools nativas, e o veredito "não reivindicar `mika:tools-list`/`tool-execute`" saía daí. Em
// 2026-09-21/22 a lacuna fechou quase inteira, e os pins negativos viraram vermelho POR ACERTO — o
// porte funcionou. Reescritos aqui com duas correções de forma, não só de número:
//
// 1. **Asserção negativa contra `registry()` nunca foi prova.** O registro é um `OnceLock` de
//    processo e `cargo test` roda em paralelo: `is_none()` só passava enquanto NENHUM teste do
//    processo registrasse aquele nome. Passava por ausência de vizinho, não por ausência de porte —
//    verde por sorte. O que sobrou são asserções POSITIVAS, e para as tools que só registram com
//    `AppHandle` (impossível num teste de unidade) a asserção de registro foi REMOVIDA em vez de
//    invertida, com o ponto de registro nomeado para quem for conferir à mão.
// 2. O pin sobre o FONTE TypeScript fica em todos: é ele que segura o denominador. Se o TS passar a
//    declarar uma tool nova, o teste falha e obriga a remedir, que é o serviço que ele presta.

// `voice/tools/voice.ts`, `person-catalog/index.ts`, `privacy/tools/index.ts` e
// `gamer-companion.feature.ts` foram removidos (backend Node deletado na migração para Tauri). Os
// "ainda declarada no catálogo TS" eram checagens de denominador da migração (concluída); as
// asserções positivas que têm par Rust seguem vivas abaixo, sem o lado TS.

/// `person_catalog` — portada em `modules/features/person_catalog/tool.rs`, sem config, logo
/// visível a este teste. O SCHEMA está completo; a persistência degrada nomeada (sem store Rust).
#[test]
fn the_person_catalog_tool_has_a_native_counterpart() {
    assert!(
        super::registry().get("person_catalog").is_some(),
        "person_catalog sumiu do registro nativo — a tool voltou a ser lacuna"
    );
}

/// `gamer-companion` — 3 das 6 tools não pedem `AppHandle` e ficam visíveis aqui: `play_minigame`
/// (sem config), `steam`/`track_gamer_mood` (antigas). `game_session`/`game_knowledge`/
/// `recommend_media` registram em `gamer::register_session_tools`, fora do alcance deste teste.
#[test]
fn gamer_companion_config_free_tools_are_native() {
    for native in ["play_minigame", "steam", "track_gamer_mood"] {
        assert!(
            super::registry().get(native).is_some(),
            "{native} sumiu do registro nativo — a tool voltou a ser lacuna"
        );
    }
}

/// `get_system_info` é registrada sozinha, fora de qualquer feature (`modules/system/tools.rs`).
#[test]
fn get_system_info_is_registered_standalone() {
    assert!(
        super::registry().get("get_system_info").is_some(),
        "get_system_info sumiu do registro nativo — a tool voltou a ser lacuna"
    );
}

// `ipc/tools.ts`, `ipc/tool-approval.ts` e `bootstrap/tools.ts` foram removidos (backend Node
// deletado na migração para Tauri). Os quatro invariantes que essas leituras pinavam —
// `tool-execute` chama o handler real (não um catálogo), `tools-list` lê o registro ao vivo, o
// registro só existe depois de features reais registradas no boot, e a aprovação pendente que
// `tool-approval-response` resolve nasce dentro do handler de `tool-execute` — são hoje
// comportamento do PRÓPRIO Rust (`super::registry()`, `native.rs`, `approval.rs`), sem um segundo
// lado TS para divergir. O gate fail-closed (sem janela, ou sem aprovação explícita) que
// `approval_gate_fails_closed_without_a_window_or_without_explicit_approval` pinava contra o TS
// continua coberto — agora direto sobre o gate Rust — por `approval::tests::denies_by_default_without_a_handler`,
// `denies_when_the_handler_denies` e `denies_on_timeout_instead_of_hanging_forever`
// (`modules/tools/approval.rs`).
