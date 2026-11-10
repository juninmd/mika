//! Namespaces de configuração do módulo `user`, portados dos `*.schema.ts` correspondentes.
//!
//! Fontes: `user.schema.ts` (aba, sem campos próprios), `user-identity-config.schema.ts`,
//! `user-interests-config.schema.ts` e `user-relations-config.schema.ts`. Todos os grupos gravam
//! sob o mesmo prefixo `USER_CONFIG_KEY` ('user'), por isso viram um único `Namespace`.

use super::super::registry::Namespace;
use super::super::schema::{choice, int_range, text, FieldUi, UiOption};

pub fn namespaces() -> Vec<Namespace> {
    vec![Namespace {
        name: "user",
        fields: vec![
            // --- user-identity-config.schema.ts: UserIdentityGroupSchema ---
            text("name", "").with_ui(FieldUi::new("settings.user.field.name.label")),
            text("nickname", "").with_ui(FieldUi::new("settings.user.field.nickname.label")),
            text("birthDate", "").with_ui(FieldUi::new("settings.user.field.birthDate.label")),
            // `age` é `z.number().int().min(1).max(150).optional()` SEM `.default()` no original —
            // o "vazio" real é `undefined` (ver `handlers/profile-config.ts`, que grava
            // `undefined` ao limpar este campo especificamente), estado que o modelo de schema em
            // Rust não representa. Default escolhido é o mínimo válido da própria faixa (1), para
            // não violar `int_range` com um valor fora dela.
            int_range("age", 1, 1, 150).with_ui(FieldUi::new("settings.user.field.age.label")),
            choice("gender", "", &["masculino", "feminino", "outro", ""]).with_ui(
                FieldUi::new("settings.user.field.gender.label").options(&[
                    UiOption {
                        label: "settings.user.field.gender.option.unset",
                        value: "",
                    },
                    UiOption {
                        label: "settings.user.field.gender.option.male",
                        value: "masculino",
                    },
                    UiOption {
                        label: "settings.user.field.gender.option.female",
                        value: "feminino",
                    },
                    UiOption {
                        label: "settings.user.field.gender.option.other",
                        value: "outro",
                    },
                ]),
            ),
            // --- user-identity-config.schema.ts: UserLocationGroupSchema ---
            text("country", "").with_ui(FieldUi::new("settings.user.field.country.label")),
            text("state", "").with_ui(FieldUi::new("settings.user.field.state.label")),
            text("city", "").with_ui(FieldUi::new("settings.user.field.city.label")),
            text("timezone", "").with_ui(
                FieldUi::new("settings.user.field.timezone.label")
                    .helper("settings.user.field.timezone.helperText"),
            ),
            // --- user-interests-config.schema.ts: UserProfessionalGroupSchema ---
            text("profession", "").with_ui(FieldUi::new("settings.user.field.profession.label")),
            text("company", "").with_ui(FieldUi::new("settings.user.field.company.label")),
            text("role", "").with_ui(FieldUi::new("settings.user.field.role.label")),
            text("skills", "").with_ui(
                FieldUi::new("settings.user.field.skills.label")
                    .control("textarea")
                    .described("settings.user.field.skills.description"),
            ),
            // --- user-interests-config.schema.ts: UserInterestsGroupSchema ---
            text("hobbies", "").with_ui(
                FieldUi::new("settings.user.field.hobbies.label")
                    .control("textarea")
                    .described("settings.user.field.hobbies.description"),
            ),
            text("favoriteGames", "")
                .with_ui(FieldUi::new("settings.user.field.favoriteGames.label").control("textarea")),
            text("favoriteAnime", "")
                .with_ui(FieldUi::new("settings.user.field.favoriteAnime.label").control("textarea")),
            text("favoriteMusic", "")
                .with_ui(FieldUi::new("settings.user.field.favoriteMusic.label").control("textarea")),
            text("favoriteFood", "").with_ui(FieldUi::new("settings.user.field.favoriteFood.label")),
            // --- user-relations-config.schema.ts: UserImportantPeopleGroupSchema ---
            text("partnerName", "").with_ui(FieldUi::new("settings.user.field.partnerName.label")),
            // O enum tem 8 variantes ('marida' incluído), mas as `options` de UI do original só
            // listam 7 — assimetria já existente no schema TypeScript, copiada como está.
            // Chave na MESMA linha do `choice(` — o gate de paridade lê campo por regex e não
            // enxerga uma chave que abra em linha própria.
            choice("partnerRelationship",
                "",
                &[
                    "",
                    "esposa",
                    "namorada",
                    "noiva",
                    "companheira",
                    "marida",
                    "marido",
                    "parceira",
                ],
            )
            .with_ui(
                FieldUi::new("settings.user.field.partnerRelationship.label").options(&[
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.unset",
                        value: "",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.wife",
                        value: "esposa",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.girlfriend",
                        value: "namorada",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.fiancee",
                        value: "noiva",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.companion",
                        value: "companheira",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.husband",
                        value: "marido",
                    },
                    UiOption {
                        label: "settings.user.field.partnerRelationship.option.partner",
                        value: "parceira",
                    },
                ]),
            ),
            text("partnerLikes", "")
                .with_ui(FieldUi::new("settings.user.field.partnerLikes.label").control("textarea")),
            text("partnerNotes", "")
                .with_ui(FieldUi::new("settings.user.field.partnerNotes.label").control("textarea")),
            // --- user-relations-config.schema.ts: UserPersonalityGroupSchema ---
            text("personalityTraits", "").with_ui(
                FieldUi::new("settings.user.field.personalityTraits.label")
                    .control("textarea")
                    .described("settings.user.field.personalityTraits.description"),
            ),
            choice("communicationStyle",
                "",
                &["direto", "descontraído", "formal", "técnico", ""],
            )
            .with_ui(
                FieldUi::new("settings.user.field.communicationStyle.label").options(&[
                    UiOption {
                        label: "settings.user.field.communicationStyle.option.default",
                        value: "",
                    },
                    UiOption {
                        label: "settings.user.field.communicationStyle.option.direct",
                        value: "direto",
                    },
                    UiOption {
                        label: "settings.user.field.communicationStyle.option.casual",
                        value: "descontraído",
                    },
                    UiOption {
                        label: "settings.user.field.communicationStyle.option.formal",
                        value: "formal",
                    },
                    UiOption {
                        label: "settings.user.field.communicationStyle.option.technical",
                        value: "técnico",
                    },
                ]),
            ),
            text("communicationNotes", "").with_ui(
                FieldUi::new("settings.user.field.communicationNotes.label")
                    .control("textarea")
                    .described("settings.user.field.communicationNotes.description"),
            ),
            // --- user-relations-config.schema.ts: UserBioGroupSchema ---
            text("bio", "").with_ui(
                FieldUi::new("settings.user.field.bio.label")
                    .control("textarea")
                    .described("settings.user.field.bio.description"),
            ),
            text("notes", "").with_ui(
                FieldUi::new("settings.user.field.notes.label")
                    .control("textarea")
                    .described("settings.user.field.notes.description"),
            ),
            text("learnedByLLM", "").with_ui(
                FieldUi::new("settings.user.field.learnedByLLM.label")
                    .control("textarea")
                    .described("settings.user.field.learnedByLLM.description"),
            ),
        ],
    }]
}
