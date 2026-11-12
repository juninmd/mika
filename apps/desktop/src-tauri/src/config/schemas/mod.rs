//! Os schemas de configuração, fonte única desde que o backend Node saiu (`b0258eee`) e os
//! últimos `*.schema.ts` (os 5 de `character-schema`) foram apagados junto com o legado Node.

mod brain;
mod character;
mod gamer;
mod llm;
pub(crate) mod llm_defaults;
pub(crate) mod cloud_stt;
pub(crate) mod provider_endpoints;
mod misc;
mod user;
mod vision;
mod voice;
pub(crate) mod voice_voices;

use super::registry::{Namespace, SchemaRegistry};
use super::schema::{boolean, choice, float, int_range, text, FieldUi};

/// `apps/desktop/src/main/services/memory/memory.schema.ts`
///
/// **`backend`/`redisUrl` removidos nesta rodada.** Nenhum backend Redis existe no port Rust
/// (`modules/memory/mod.rs` documenta a decisão: só o backend embarcado `rusqlite` — ver o
/// cabeçalho do módulo, "EmbeddedMemoryStore foi escolhido, e não o backend Redis"). Os dois campos
/// nunca tiveram leitor de produção (`RedisRunner` era só TypeScript); mantê-los prometia uma troca
/// de backend que a UI não entrega.
fn memory() -> Namespace {
    Namespace {
        name: "memory",
        fields: vec![
            boolean("enabled", true).with_ui(
                FieldUi::new("settings.memory.field.enabled.label")
                    .described("settings.memory.field.enabled.description"),
            ),
            // `z.number()` sem `.int()` no original — fica Float para não recusar 0.5 que o
            // TypeScript aceitava. Paridade vale nos dois sentidos: apertar demais também quebra.
            float("shortTermTtlHours", 168.0).with_ui(
                FieldUi::new("settings.memory.field.shortTermTtlHours.label")
                    .described("settings.memory.field.shortTermTtlHours.description"),
            ),
            float("longTermTtlDays", 365.0).with_ui(
                FieldUi::new("settings.memory.field.longTermTtlDays.label")
                    .described("settings.memory.field.longTermTtlDays.description"),
            ),
            int_range("episodicListLimit", 500, 1, 5000).with_ui(
                FieldUi::new("settings.memory.field.episodicListLimit.label")
                    .described("settings.memory.field.episodicListLimit.description"),
            ),
        ],
    }
}

/// `telemetry` NÃO tinha schema Zod do lado TypeScript — o serviço gravava
/// `telemetry.enabled`, `telemetry.sessionId` e `telemetry.optInShown` direto no store, e só os
/// dois primeiros existiam em `config-keys.enum.ts`.
///
/// Escrever caminho não declarado é justamente a classe de defeito que já custou caro aqui (o Zod
/// do router apagava o campo em silêncio). O port declara os três, então eles passam a ser
/// validados de verdade. Não é mudança de comportamento visível: os mesmos caminhos, os mesmos
/// defaults que os `?? false` do serviço já aplicavam.
///
/// Sem UI de propósito: o opt-in de telemetria tem tela própria, não é um campo de Settings.
fn telemetry() -> Namespace {
    Namespace {
        name: "telemetry",
        fields: vec![
            boolean("enabled", false),
            boolean("optInShown", false),
            text("sessionId", ""),
        ],
    }
}

/// Aceite do EULA e da política de privacidade (`modules/legal.rs` calcula a versão; o diálogo
/// `LegalConsent` do renderer grava). Sem schema TypeScript, como `setup` e `telemetry`: o espelho
/// do lado TS é só o `ConfigKey`. Default vazio = "nunca aceitou" — vale para o primeiro boot e para
/// quem já usava o app antes do aceite existir.
fn legal() -> Namespace {
    Namespace {
        name: "legal",
        fields: vec![
            text("acceptedVersion", ""),
            text("acceptedAt", ""),
            // "Tenho 18 anos ou mais" — marcado no mesmo aceite (`LegalConsent`). Sem ele o gate reabre
            // e o vínculo `companion` cai para `friend` (`chat::persona_relationship`).
            boolean("ageConfirmed", false),
        ],
    }
}

/// `setup`, como `telemetry`, NÃO tem schema no TypeScript — e aqui isso não é descuido de lá, é
/// arquitetura: `main/ipc/settings-paths.ts` libera as duas chaves por uma allowlist estática, e o
/// Zod raiz de `schema/schema-registry.ts` é `.passthrough()`, então a chave top-level `setup`
/// sobrevive ao `safeParse` e é persistida sem validação nenhuma. O Rust não tem passthrough:
/// `config/registry.rs` recusa caminho não declarado, e por isso TODA gravação do wizard vinha
/// `success: false`.
///
/// **O efeito era visível e nada sutil:** o onboarding abria com a faixa vermelha "Não foi possível
/// concluir o setup agora" antes de qualquer clique (`use-wizard-state.ts` grava o passo assim que
/// restaura), e o progresso não persistia — reabrir voltava para a etapa 1. Achado EXECUTANDO o
/// app; nenhum teste podia ver, porque o defeito é a interseção de dois contratos que só se
/// encontram em runtime.
///
/// Os valores abaixo vêm do comportamento observado, não de um `z.object` que não existe:
/// `wizardCompleted` é tratado como pendente por `!== true` (`main/index.ts`, `sidecar/index.ts`),
/// e os cinco passos são `wizard-data.ts::Step` / `wizard-app-saves.ts::WIZARD_STEPS`.
///
/// `"done"` entra no enum mesmo NUNCA sendo gravado (`use-wizard-state.ts` retorna antes de salvar
/// quando `step === 'done'`): o filtro de restauração o aceita, e um config legado ou um e2e que o
/// grave seria recusado se ele faltasse aqui.
///
/// Sem UI de propósito — o wizard tem tela própria, não é campo de Settings.
fn setup() -> Namespace {
    Namespace {
        name: "setup",
        fields: vec![
            boolean("wizardCompleted", false),
            choice("wizardCurrentStep", "language", &["language", "avatar", "llm", "tts", "stt", "embedding", "privacy", "identity", "done"]),
        ],
    }
}

/// `apps/desktop/src/main/services/app-config.schema.ts`, cujo `SupportedLocaleSchema` vem de
/// `packages/sdk/src/contracts/schemas/config.schema.ts`.
///
/// A lista tem de ser os DEZ locales de `SUPPORTED_LOCALES`. A primeira versão deste port listou
/// só dois, e o efeito seria mudo e severo: quem usa o app em japonês ou espanhol teria o próprio
/// idioma recusado como valor inválido na primeira gravação de config.
///
/// `check:config-schema-parity` lê cada `Namespace` por regex: `name: "...", fields: vec![...]`
/// tem de ficar colado (só espaço em branco entre as duas partes, nunca comentário), e o `]` de
/// fechamento do array precisa vir isolado em linha própria — um `vec![choice(...)]` cujo `]` final
/// ficasse colado no `)` da mesma linha escapava dessa detecção: a busca (não gulosa) continuava
/// até o PRÓXIMO fechamento bem formatado — o de `memory()` logo abaixo — e vazava os campos dela
/// para dentro deste namespace.
fn app() -> Namespace {
    Namespace {
        name: "app",
        fields: vec![
            choice(
                "locale",
                "pt-BR",
                &["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"],
            ),
            // Governador de energia (`modules/system/governor`): Automático escala os laços de
            // fundo, o fps do avatar e as threads ONNX pelo estado (ativo/ocioso/jogo/bateria).
            choice("performanceProfile", "auto", &["auto", "economy", "quality"]),
            // Aceleração dos modelos ONNX locais (`modules/inference/accel.rs`): Automática decide
            // por modelo e pelo governador; GPU/CPU forçam (modelos minúsculos ficam na CPU sempre).
            choice("inferenceAcceleration", "auto", &["auto", "gpu", "cpu"]),
            // Tela cheia da janela do avatar (`fullscreen.rs`): o interruptor de Configurações, o F11 e a
            // paleta gravam esta chave, e o observador aplica na janela. Reaplicada no boot.
            boolean("fullscreen", false),
            // Atualização automática (`modules/update/auto.rs`): só avisa, ou baixa e instala sozinha
            // quando o PC fica ocioso. Padrão `notify`: quem decide instalar é o dono do PC, e o
            // aviso chega no mesmo boot em que a versão nova é encontrada. Controle em
            // Configurações → Atualizações.
            choice("autoUpdate", "notify", &["off", "notify", "install"]),
        ],
    }
}

/// Os namespaces já portados. Cada fase acrescenta uma linha.
pub fn registry() -> SchemaRegistry {
    let mut namespaces = vec![app(), memory(), setup(), telemetry(), legal()];

    namespaces.extend(brain::namespaces());
    namespaces.extend(character::namespaces());
    namespaces.extend(gamer::namespaces());
    namespaces.extend(llm::namespaces());
    namespaces.extend(misc::namespaces());
    namespaces.extend(user::namespaces());
    namespaces.extend(vision::namespaces());
    namespaces.extend(voice::namespaces());

    SchemaRegistry::new(namespaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every `ConfigKey` the renderer can save must be declared here: `validate` refuses an
    /// undeclared path, and the wizard's `llm.ollamaModel` save failing that way stalled setup.
    #[test]
    fn every_typescript_config_key_is_declared() {
        let source = include_str!("../../../../../../packages/sdk/src/contracts/schemas/config-keys.enum.ts");
        let registry = registry();
        let undeclared: Vec<&str> = source
            .lines()
            .filter_map(|line| line.split_once("= '").map(|(_, rest)| rest))
            .filter_map(|rest| rest.split_once('\'').map(|(path, _)| path))
            .filter(|path| path.contains('.'))
            .filter(|path| {
                registry.validate(path, &serde_json::Value::Null).is_err_and(|e| e.contains("não declarado"))
            })
            .collect();
        assert!(undeclared.is_empty(), "ConfigKey sem schema Rust: {undeclared:?}");
    }

    /// The audio device picker saves literal paths, outside `ConfigKey`; refused, the pick reverted.
    #[test]
    fn the_audio_device_picks_are_declared() {
        let source = include_str!("../../../../src/renderer/src/hooks/useAudioDevices.ts");
        let registry = registry();
        for path in ["audio.inputDeviceId", "audio.outputDeviceId"] {
            assert!(source.contains(&format!("'{path}'")), "{path} no longer saved by the hook");
            assert!(registry.validate(path, &json!("device-1")).is_ok(), "{path} not declared");
        }
    }

    /// `backend`/`redisUrl` foram removidos nesta rodada (nenhum backend Redis existe no port
    /// Rust) — este teste passa a fixar a AUSÊNCIA dos dois, não mais os defaults deles.
    #[test]
    fn memory_defaults_match_the_typescript_schema() {
        let defaults = registry().defaults();

        assert_eq!(defaults["memory"]["enabled"], json!(true));
        assert!(defaults["memory"].get("backend").is_none(), "memory.backend não existe: nenhum backend Redis no port Rust");
        assert!(defaults["memory"].get("redisUrl").is_none(), "memory.redisUrl não existe: nenhum backend Redis no port Rust");
        assert_eq!(defaults["memory"]["shortTermTtlHours"], json!(168.0));
        assert_eq!(defaults["memory"]["longTermTtlDays"], json!(365.0));
        assert_eq!(defaults["memory"]["episodicListLimit"], json!(500));
    }

    /// Toda etapa do wizard tem de ser um valor ACEITO. Sem este namespace, `ConfigStore::set`
    /// recusava `setup.wizardCurrentStep` por não declarado, o onboarding abria com a faixa
    /// vermelha "Não foi possível concluir o setup agora" antes de qualquer clique, e o progresso
    /// não persistia entre execuções — achado executando o app, invisível para a suíte.
    ///
    /// A lista é lida do FONTE que a define, não copiada: uma etapa nova no TypeScript sem a linha
    /// correspondente aqui volta a recusar a gravação, com o mesmo sintoma e nenhum erro de
    /// compilação.
    #[test]
    fn every_wizard_step_the_renderer_can_persist_is_accepted() {
        let steps_source = include_str!("../../../../src/renderer/src/wizard-app-saves.ts");
        let start = steps_source.find("WIZARD_STEPS").expect("WIZARD_STEPS sumiu de wizard-app-saves.ts");
        // Do `=` em diante, NÃO do primeiro `[`: a anotação de tipo é `Step[]`, e cortar no primeiro
        // colchete devolvia a lista VAZIA — o teste acusou 0 etapas lidas em vez de passar por
        // vacuidade, que é justamente o que o piso de alcance existe para fazer.
        let list = &steps_source[start..];
        let list = &list[list.find('=').expect("atribuição de WIZARD_STEPS")..];
        let list = &list[list.find('[').expect("lista de etapas")..list.find(']').expect("fim da lista")];

        let registry = registry();
        let mut checked = 0usize;
        // A lista de etapas pode ter vindo do biome multilinha: os cortes precisam aceitar as
        // brancas internas de indentação, não só as externas.
        for step in list.split(',') {
            let step = step.trim().trim_matches(['[', '\'', '"', ' ', '\r', '\n', '\t']);
            if step.is_empty() {
                continue;
            }
            assert!(
                registry.validate("setup.wizardCurrentStep", &json!(step)).is_ok(),
                "etapa '{step}' do wizard é recusada pelo schema — a gravação volta success:false e o onboarding mostra erro"
            );
            checked += 1;
        }
        assert_eq!(checked, 9, "esperava as 9 etapas de WIZARD_STEPS (com escuta), li {checked} — a extração quebrou");

        // NEGATIVO: o enum ainda recusa o que não é etapa, senão `choice` viraria `text`.
        assert!(registry.validate("setup.wizardCurrentStep", &json!("qualquer-coisa")).is_err());
        assert_eq!(registry.defaults()["setup"]["wizardCurrentStep"], json!("language"));
        assert_eq!(registry.defaults()["setup"]["wizardCompleted"], json!(false));
    }

    /// PRIVACY.md §3: o que manda dado para fora (busca web, consulta ao catálogo de pessoas) ou
    /// controla o computador do usuário NÃO nasce ligado num perfil novo. Opt-in explícito, no
    /// painel de cada recurso — o assistente não liga nenhum deles por baixo dos panos.
    #[test]
    fn features_that_leave_the_machine_or_drive_it_are_off_for_a_new_profile() {
        let defaults = registry().defaults();

        assert_eq!(defaults["browser"]["enabled"], json!(false), "busca web");
        assert_eq!(defaults["person-catalog"]["webEnrichment"], json!(false), "enriquecimento Wikipédia/buscadores");
        assert_eq!(defaults["computer-use"]["enabled"], json!(false), "controle do computador");
        // O que é 100% local continua ligado: o catálogo em si e a extração de menções.
        assert_eq!(defaults["person-catalog"]["enabled"], json!(true));
    }

    #[test]
    fn telemetry_defaults_match_the_service_fallbacks() {
        let defaults = registry().defaults();

        assert_eq!(defaults["telemetry"]["enabled"], json!(false));
        assert_eq!(defaults["telemetry"]["optInShown"], json!(false));
    }

    /// Ninguém nasce tendo aceito: o default vazio é o que faz o primeiro boot (e o usuário que já
    /// existia antes do aceite) cair no diálogo de EULA/privacidade.
    #[test]
    fn legal_acceptance_starts_empty_and_takes_a_version_and_a_timestamp() {
        let registry = registry();
        let defaults = registry.defaults();

        assert_eq!(defaults["legal"]["acceptedVersion"], json!(""));
        assert_eq!(defaults["legal"]["acceptedAt"], json!(""));
        assert_eq!(defaults["legal"]["ageConfirmed"], json!(false));
        assert!(registry.validate("legal.ageConfirmed", &json!(true)).is_ok());
        assert!(registry.validate("legal.ageConfirmed", &json!("sim")).is_err());
        assert!(registry.validate("legal.acceptedVersion", &json!("2e869b7c944d937b")).is_ok());
        assert!(registry.validate("legal.acceptedAt", &json!("2026-10-06T12:00:00.000Z")).is_ok());
        assert!(registry.validate("legal.acceptedVersion", &json!(7)).is_err());
    }

    /// Os DEZ locales de `SUPPORTED_LOCALES`, não um subconjunto. Um port que listasse menos
    /// recusaria o idioma do próprio usuário na primeira gravação de config, sem nada na tela
    /// explicando o motivo.
    #[test]
    fn locale_accepts_every_supported_value() {
        let registry = registry();

        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            assert!(
                registry.validate("app.locale", &json!(locale)).is_ok(),
                "{locale} deveria ser aceito"
            );
        }

        assert!(registry.validate("app.locale", &json!("xx-XX")).is_err());
    }

    /// As chaves de i18n precisam chegar ao renderer; sem elas o painel desenha rótulos vazios e
    /// os gates de UI continuam verdes.
    ///
    /// `fields[1]` era `backend` (`choice`/"select") até esta rodada — removido junto de `redisUrl`
    /// (nenhum backend Redis existe no port Rust). O segundo campo agora é `shortTermTtlHours`, um
    /// número puro, então a asserção de tipo passa a ser sobre esse campo.
    #[test]
    fn declarations_carry_the_translation_keys() {
        let registry = registry();
        let json = serde_json::to_value(registry.declarations()).unwrap();

        let memory = json
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["namespace"] == "memory")
            .expect("namespace memory ausente");

        assert_eq!(memory["fields"][0]["ui"]["label"], "settings.memory.field.enabled.label");
        assert_eq!(memory["fields"][1]["ui"]["label"], "settings.memory.field.shortTermTtlHours.label");
    }
}
