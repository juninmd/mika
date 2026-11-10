//! Declaração de settings das abas `voice` e `stt` para `crate::feature_registry::shared()` — port
//! de `voice.schema.ts::VOICE_SETTINGS`/`STT_SETTINGS` (duas chamadas de `defineFeatureSchema` no
//! MESMO arquivo). Sem isto `mika:settings-declarations` publica 13 das 15 abas do TS — as duas
//! telas de voz somem da UI. Campos e mapa de agrupamento moram em [`feature_fields`] só para caber
//! no teto de 300 linhas; ver o cabeçalho daquele arquivo para por que ele NÃO importa
//! `features::settings_declaration::declare_fields` (violaria "núcleo nunca importa de feature").
//!
//! ## Duas abas, UM namespace — e por que o grupo "outros" mora só em `voice`
//!
//! `declare_fields` (local, [`feature_fields`]) devolve os MESMOS 55 campos do namespace `voice`
//! para as duas chamadas abaixo. [`declaration_voice`] reivindica 47 (`VOICE_GROUPS`: runtime + 6
//! grupos de provedor de TTS); [`declaration_stt`] reivindica os 8 do ASR (`STT_GROUP`).
//!
//! Se as DUAS calculassem leftover e emitissem "outros", cada campo do ASR apareceria como "não
//! agrupado" dentro da aba `voice` — e vice-versa: cada chamada só enxerga o namespace inteiro, não
//! sabe que a OUTRA já reivindicou o resto. Só [`declaration_voice`] tem essa lógica, e desconta as
//! chaves de `STT_GROUP` do próprio `by_key` ANTES de decidir se "outros" existe. `declaration_stt`
//! nunca cria um: se `voice-stt-config.schema.ts` ganhar campo sem entrar em `STT_GROUP`, ele sobra
//! no `by_key` de `declaration_voice` e cai no "outros" DAQUELA aba — visível, nunca mudo.
//!
//! ## Contagem: 55, não 63
//!
//! Uma rodada de "só deixa o que faz sentido" cortou 8 campos sem leitor Rust nenhum (dead config
//! da UI) e o grupo `kokoro` (provedor cortado em `dispatch.rs::CUT_PROVIDERS`, sem sair do enum
//! `ttsProvider` — ver `voice.rs::runtime_fields`). `config/schemas/
//! voice.rs::ports_every_field_from_the_nine_source_files` prova 55 (17 runtime + 8 elevenlabs + 5
//! azure + 7 google + 4 openai + 3 piper + 3 supertonic + 8 stt) — 47 (`VOICE_GROUPS`) +
//! 8 (`STT_GROUP`) = 55.

mod feature_fields;

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::config::registry::SchemaRegistry;
use crate::feature_registry::{self, FeatureRegistration};

use feature_fields::{build_group, declare_fields, key_of, NAMESPACE, STT_GROUP, VOICE_GROUPS};

fn other_group(title: &'static str, by_key: HashMap<String, Value>) -> Option<Value> {
    if by_key.is_empty() {
        return None;
    }
    let mut leftover: Vec<(String, Value)> = by_key.into_iter().collect();
    leftover.sort_by(|a, b| a.0.cmp(&b.0));
    Some(json!({
        "title": title,
        "groupPrefix": NAMESPACE,
        "fields": leftover.into_iter().map(|(_, field)| field).collect::<Vec<_>>(),
    }))
}

/// `VOICE_SETTINGS` (`voice.schema.ts`). Sem `pluginId`/`isCore` — o registro injeta os dois. Sem
/// `enabledConfigPath`: nenhum grupo declara campo chamado `enabled` (é `ttsEnabled`), mesma regra
/// de `inferEnabledConfigPath` no lado TS.
pub fn declaration_voice(schema: &SchemaRegistry) -> Option<Value> {
    let mut by_key: HashMap<String, Value> =
        declare_fields(schema)?.into_iter().map(|field| (key_of(&field), field)).collect();

    let mut groups: Vec<Value> = VOICE_GROUPS.iter().map(|spec| build_group(spec, &mut by_key)).collect();

    // As 8 chaves do ASR pertencem à aba `stt` — descontadas aqui SEM virar grupo (cabeçalho acima).
    for key in STT_GROUP.keys {
        by_key.remove(*key);
    }

    if let Some(other) = other_group("settings.voiceTts.group.other", by_key) {
        groups.push(other);
    }

    Some(json!({
        "tabId": "voice",
        "tabLabel": "settings.voiceTts.tab.label",
        "tabDescription": "settings.voiceTts.tab.description",
        "tabIcon": "Volume2",
        "tabGroup": "settings.voice.tab.group",
        "component": "TTSProviderPanel",
        "accentColor": "indigo",
        "groups": groups,
    }))
}

/// `STT_SETTINGS` (`voice.schema.ts`).
pub fn declaration_stt(schema: &SchemaRegistry) -> Option<Value> {
    let mut by_key: HashMap<String, Value> =
        declare_fields(schema)?.into_iter().map(|field| (key_of(&field), field)).collect();

    let groups: Vec<Value> = vec![build_group(&STT_GROUP, &mut by_key)];

    Some(json!({
        "tabId": "stt",
        "tabLabel": "settings.voiceStt.tab.label",
        "tabDescription": "settings.voiceStt.tab.description",
        "tabIcon": "Mic",
        "tabGroup": "settings.voice.tab.group",
        "component": "STTProviderPanel",
        "accentColor": "rose",
        "groups": groups,
    }))
}

/// Registra as DUAS abas no `FeatureRegistry` compartilhado sob `mika.voice`. Chame UMA VEZ no boot.
pub fn register_feature() {
    let schema = crate::config::schemas::registry();
    let settings: Vec<Value> = [declaration_voice(&schema), declaration_stt(&schema)].into_iter().flatten().collect();
    if settings.is_empty() {
        log::warn!("namespace 'voice' ausente do SchemaRegistry — abas voice/stt não registradas");
        return;
    }
    feature_registry::shared().register(
        "mika.voice",
        FeatureRegistration { enabled: true, settings, ..Default::default() },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> SchemaRegistry {
        crate::config::schemas::registry()
    }

    fn field_paths(group: &Value) -> Vec<String> {
        group["fields"].as_array().unwrap().iter().map(|f| f["configPath"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn voice_tab_has_the_seven_ts_groups_in_order_no_leftover() {
        let d = declaration_voice(&schema()).unwrap();
        assert_eq!(d["tabId"], json!("voice"));
        assert_eq!(d["tabIcon"], json!("Volume2"));
        assert_eq!(d["accentColor"], json!("indigo"));
        assert_eq!(d["component"], json!("TTSProviderPanel"));
        assert_eq!(d["tabGroup"], json!("settings.voice.tab.group"));
        assert!(d.get("enabledConfigPath").is_none());

        let groups = d["groups"].as_array().unwrap();
        // 7, nunca 8: se aparecer um 8º grupo é porque "outros" não ficou vazio. `kokoro` saiu
        // (provedor cortado, `dispatch.rs::CUT_PROVIDERS`) sem deixar campo órfão para trás.
        assert_eq!(groups.len(), 7, "grupo 'outros' apareceu na aba voice — algum campo não foi reivindicado");

        let titles: Vec<&str> = groups.iter().map(|g| g["title"].as_str().unwrap()).collect();
        assert_eq!(
            titles,
            vec![
                "settings.voiceTts.group.runtime",
                "settings.voiceTts.group.elevenlabs",
                "settings.voiceTts.group.supertonic",
                "settings.voiceTts.group.piper",
                "settings.voiceTts.group.azure",
                "settings.voiceTts.group.google",
                "settings.voiceTts.group.openai",
            ]
        );

        let counts: Vec<usize> = groups.iter().map(|g| g["fields"].as_array().unwrap().len()).collect();
        assert_eq!(counts, vec![17, 9, 4, 3, 7, 7, 5]);
        assert_eq!(counts.iter().sum::<usize>(), 52);
    }

    #[test]
    fn voice_tab_runtime_group_has_no_visible_when() {
        let d = declaration_voice(&schema()).unwrap();
        assert!(d["groups"][0].get("visibleWhen").is_none());
    }

    /// `visibleWhen: { configPath: 'voice.ttsProvider', equals: <provider> }` em cada grupo de
    /// provedor — sem ele os 7 grupos aparecem juntos na tela em vez de trocar com `ttsProvider`.
    #[test]
    fn voice_tab_provider_groups_carry_visible_when() {
        let d = declaration_voice(&schema()).unwrap();
        let groups = d["groups"].as_array().unwrap();
        let expected = [
            (1, "elevenlabs"),
            (2, "supertonic"),
            (3, "piper"),
            (4, "azure-tts"),
            (5, "google-tts"),
            (6, "openai-tts"),
        ];
        for (index, equals) in expected {
            assert_eq!(groups[index]["visibleWhen"]["configPath"], json!("voice.ttsProvider"), "grupo {index}");
            assert_eq!(groups[index]["visibleWhen"]["equals"], json!(equals), "grupo {index}");
        }
    }

    #[test]
    fn voice_tab_elevenlabs_group_carries_the_test_action() {
        let d = declaration_voice(&schema()).unwrap();
        assert_eq!(d["groups"][1]["testAction"]["label"], json!("settings.voiceTts.testAction.label"));
        // Nenhum outro grupo tem `testAction` — só `elevenlabs` em `voice.schema.ts`.
        for index in [0, 2, 3, 4, 5, 6] {
            assert!(d["groups"][index].get("testAction").is_none(), "grupo {index} não devia ter testAction");
        }
    }

    #[test]
    fn stt_tab_has_the_one_asr_group_with_eight_fields_no_leftover() {
        let d = declaration_stt(&schema()).unwrap();
        assert_eq!(d["tabId"], json!("stt"));
        assert_eq!(d["tabIcon"], json!("Mic"));
        assert_eq!(d["accentColor"], json!("rose"));
        assert_eq!(d["component"], json!("STTProviderPanel"));
        assert_eq!(d["tabGroup"], json!("settings.voice.tab.group"));
        assert!(d.get("enabledConfigPath").is_none());

        let groups = d["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 1, "grupo 'outros' não devia existir na aba stt");
        assert_eq!(groups[0]["title"], json!("settings.voiceStt.group.asr"));
        assert!(groups[0].get("visibleWhen").is_none());

        let paths = field_paths(&groups[0]);
        assert_eq!(
            paths,
            vec![
                "voice.sttEnabled",
                "voice.voiceActivityThreshold",
                "voice.vadEngine",
                "voice.vadIgnoreTyping",
                "voice.inputMode",
                "voice.asrProvider",
                "voice.asrLanguage",
                "voice.whisperModel",
                "voice.groqApiKey",
                "voice.openaiApiKey",
                "voice.groqSttModel",
                "voice.openaiSttModel",
            ]
        );
    }

    /// NEGATIVO — a classe de defeito que este arquivo existe para não repetir: os 55 campos do
    /// namespace `voice` aparecem exatamente uma vez, somando as duas abas. Um campo em falta
    /// deriva de uma chave errada em `VOICE_GROUPS`/`STT_GROUP`; um campo em dobro, de uma chave
    /// repetida entre os dois.
    #[test]
    fn every_field_of_the_namespace_appears_exactly_once_across_both_tabs() {
        let all_fields = declare_fields(&schema()).unwrap();
        assert_eq!(all_fields.len(), 64, "config/schemas/voice.rs mudou de tamanho — releia antes de ajustar isto");

        let voice = declaration_voice(&schema()).unwrap();
        let stt = declaration_stt(&schema()).unwrap();

        let mut combined: Vec<String> = voice["groups"]
            .as_array()
            .unwrap()
            .iter()
            .chain(stt["groups"].as_array().unwrap())
            .flat_map(field_paths)
            .collect();
        combined.sort();

        let mut expected: Vec<String> =
            all_fields.iter().map(|f| f["configPath"].as_str().unwrap().to_string()).collect();
        expected.sort();

        assert_eq!(combined, expected);
    }

    #[test]
    fn register_feature_publishes_both_tabs_under_mika_voice() {
        register_feature();
        let registered = feature_registry::shared().get(&"mika.voice".into()).expect("mika.voice registrado");
        assert!(registered.enabled);
        let tab_ids: Vec<&str> = registered.settings.iter().map(|s| s["tabId"].as_str().unwrap()).collect();
        assert_eq!(tab_ids, vec!["voice", "stt"]);
    }

    /// Pin de ordem: `voice.schema.ts` foi removido (backend Node deletado na migração para
    /// Tauri). `tabId`/`tabIcon`/`component`/`accentColor` já são cobertos por
    /// `declaration_voice_*`/`declaration_stt_*` acima; o que só este teste protegia era a ORDEM
    /// dos grupos de provedor de TTS, que agora é medida sobre `VOICE_GROUPS`/`declaration_voice`
    /// diretamente — única fonte de verdade depois da deleção.
    #[test]
    fn voice_group_titles_are_in_the_expected_order() {
        let titles_in_order = [
            "settings.voiceTts.group.runtime",
            "settings.voiceTts.group.elevenlabs",
            "settings.voiceTts.group.supertonic",
            "settings.voiceTts.group.piper",
            "settings.voiceTts.group.azure",
            "settings.voiceTts.group.google",
            "settings.voiceTts.group.openai",
        ];

        let declaration = declaration_voice(&schema()).unwrap();
        let groups = declaration["groups"].as_array().unwrap();
        let titles: Vec<&str> = groups.iter().take(titles_in_order.len()).map(|g| g["title"].as_str().unwrap()).collect();
        assert_eq!(titles, titles_in_order);
    }
}
