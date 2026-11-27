//! Catálogo FECHADO dos modelos Phonon que o app oferece. Só o que está aqui baixa: o id vem da
//! config (`voice.phononModel`), nunca de texto livre, e cada entrada carrega repositório, revisão
//! (commit, não tag móvel) e a integridade de cada um dos quatro arquivos.
//!
//! **Licença é critério de entrada.** `phonon-40m-pt` e `phonon-40m-multilingual` são CC BY-NC 4.0
//! (uso NÃO comercial) e só baixam com o usuário autenticado — o gate do Hugging Face exige aceitar
//! os termos na página do modelo e informar um token. `phonon-31m-en` é Apache-2.0 e público. O
//! registro completo está em `LICENSE-ASSETS.md`/`THIRD-PARTY-NOTICES.md`.
//!
//! **Integridade.** Arquivos LFS (pesos e voz) conferem por SHA-256. `config.json`/`tokenizer.json`
//! não são LFS: a API do Hugging Face devolve `blobId`, o SHA-1 do blob git — a única impressão
//! verificável sem baixar o arquivo (repositório com gate não entrega nem o tamanho sem token).
//! Os valores abaixo foram medidos na API `?blobs=true` na revisão fixada (2026-11-26).

/// Modelo de fábrica: pt-BR, o idioma do produto.
pub const DEFAULT_MODEL: &str = "phonon-40m-pt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum License {
    CcByNc40,
    Apache20,
}

impl License {
    /// O aviso da UI é obrigatório para peso não comercial; não é decoração.
    pub fn is_non_commercial(self) -> bool {
        matches!(self, Self::CcByNc40)
    }
}

/// Como o arquivo baixado prova que é o arquivo medido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrity {
    Sha256(&'static str),
    GitBlobSha1(&'static str),
}

impl Integrity {
    /// Nome do arquivo no repo do Hugging Face (`config.json`, `model.q8.gguf`, ...).
    pub fn digest(self) -> &'static str {
        match self {
            Self::Sha256(value) | Self::GitBlobSha1(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FileSpec {
    pub name: &'static str,
    pub integrity: Integrity,
}

#[derive(Clone, Copy, Debug)]
pub struct PhononModel {
    /// Chave de config (`voice.phononModel`).
    pub key: &'static str,
    /// `owner/name` no Hugging Face.
    pub repo: &'static str,
    pub revision: &'static str,
    /// Idiomas que o checkpoint fala, na ordem de preferência para normalização de texto.
    pub languages: &'static [&'static str],
    pub license: License,
    pub gated: bool,
    /// Vozes registráveis. O checkpoint traz `default-voice.safetensors`, que o `ptts` registra
    /// como `default`; voz nova só entra quando o repositório publicar o arquivo correspondente.
    pub voices: &'static [&'static str],
    pub files: [FileSpec; 4],
}

const VOICES: &[&str] = &["default"];

pub const MODELS: [PhononModel; 3] = [
    PhononModel {
        key: "phonon-40m-pt",
        repo: "gradium/phonon-40m-pt",
        revision: "4cbfd742a1555d60eb5399c6d296e9f2e07389e9",
        languages: &["pt"],
        license: License::CcByNc40,
        gated: true,
        voices: VOICES,
        files: [
            FileSpec { name: "config.json", integrity: Integrity::GitBlobSha1("b45c9e49061f36c362e69aaf94fbd3be8bc5e63c") },
            FileSpec { name: "tokenizer.json", integrity: Integrity::GitBlobSha1("d24831c796c012ecb6f013e0d4d5292b23c20d40") },
            FileSpec {
                name: "model.q8.gguf",
                integrity: Integrity::Sha256("e1ab3d6a1a6835ab73ac1fc1b326c9a8238292c101d2050296413ab30dba7681"),
            },
            FileSpec {
                name: "default-voice.safetensors",
                integrity: Integrity::Sha256("842930df459e1d636cdd3414cd60ab2631ebfdff29460a922415e506b1cc5ed1"),
            },
        ],
    },
    PhononModel {
        key: "phonon-40m-multilingual",
        repo: "gradium/phonon-40m-multilingual",
        revision: "1537ce46c64f42903e0d608b132ccb073dac46d2",
        languages: &["en", "fr", "de", "es", "pt"],
        license: License::CcByNc40,
        gated: true,
        voices: VOICES,
        files: [
            FileSpec { name: "config.json", integrity: Integrity::GitBlobSha1("5ddd955511e851a1f8c744ac23b69eb6000565d5") },
            FileSpec { name: "tokenizer.json", integrity: Integrity::GitBlobSha1("e2d7bebf27916234f26883ad8e55c5219d0ae148") },
            FileSpec {
                name: "model.q8.gguf",
                integrity: Integrity::Sha256("96fa067813244e0c6c5997b98ec2596b65ec8574b14916b48e97db056ff97e27"),
            },
            FileSpec {
                name: "default-voice.safetensors",
                integrity: Integrity::Sha256("dc537478eaadc1a5257fcda2e3501ab81e14c111a5d95a5838f350db2afd0f1a"),
            },
        ],
    },
    PhononModel {
        key: "phonon-31m-en",
        repo: "gradium/phonon-31m-en",
        revision: "92741c5b07d085ff840e26fea8ce43110b87852f",
        languages: &["en"],
        license: License::Apache20,
        gated: false,
        voices: VOICES,
        files: [
            FileSpec { name: "config.json", integrity: Integrity::GitBlobSha1("11cfeb2513febe31358a8ff942093e0b2c28710a") },
            FileSpec { name: "tokenizer.json", integrity: Integrity::GitBlobSha1("70d39980cb2154bd364dbed8424b1343923a03e2") },
            FileSpec {
                name: "model.q8.gguf",
                integrity: Integrity::Sha256("a7d9aad2cd6dc66473f24a08bc6d2939030ef4d751ef82349361fbef3272dbae"),
            },
            FileSpec {
                name: "default-voice.safetensors",
                integrity: Integrity::Sha256("dc537478eaadc1a5257fcda2e3501ab81e14c111a5d95a5838f350db2afd0f1a"),
            },
        ],
    },
];

/// `None` para chave fora do catálogo — o chamador devolve erro nomeado, nunca cai num default
/// silencioso (um modelo trocado por baixo mudaria a voz que o usuário escolheu).
pub fn model(key: &str) -> Option<&'static PhononModel> {
    MODELS.iter().find(|model| model.key == key)
}

/// URL de um arquivo na revisão FIXA. `main`/tag nunca aparecem aqui.
pub fn url(model: &PhononModel, file: &str) -> String {
    format!("https://huggingface.co/{}/resolve/{}/{}", model.repo, model.revision, file)
}

/// Página do modelo (termos de uso, aceite do gate) — vai no texto do erro de 401/403.
pub fn page(model: &PhononModel) -> String {
    format!("https://huggingface.co/{}", model.repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_commit_sha(value: &str) -> bool {
        value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
    }

    #[test]
    fn urls_are_pinned_to_the_revision_and_never_to_main() {
        for model in &MODELS {
            for file in &model.files {
                let url = url(model, file.name);
                assert!(url.contains(model.revision), "{url}");
                assert!(!url.contains("/main/"), "{url}");
                assert_eq!(url, format!("https://huggingface.co/{}/resolve/{}/{}", model.repo, model.revision, file.name));
            }
        }
    }

    #[test]
    fn every_revision_and_digest_is_a_real_hex_hash() {
        for model in &MODELS {
            assert!(is_commit_sha(model.revision), "{}: revisão não é um commit de 40 hex", model.key);
            for file in &model.files {
                let digest = file.integrity.digest();
                let expected_len = match file.integrity {
                    Integrity::Sha256(_) => 64,
                    Integrity::GitBlobSha1(_) => 40,
                };
                assert_eq!(digest.len(), expected_len, "{}: {}", model.key, file.name);
                assert!(digest.chars().all(|c| c.is_ascii_hexdigit()), "{}: {}", model.key, file.name);
            }
        }
    }

    #[test]
    fn every_model_carries_the_four_files_the_checkpoint_needs() {
        for model in &MODELS {
            let names: Vec<&str> = model.files.iter().map(|file| file.name).collect();
            assert_eq!(names, ["config.json", "tokenizer.json", "model.q8.gguf", "default-voice.safetensors"]);
        }
    }

    /// Gate do Hugging Face não é detalhe de UI: sem token a instalação falha. O teste fixa a
    /// relação medida na API (só o 31m-en é público) para ninguém trocar `gated` sem perceber.
    #[test]
    fn only_the_apache_model_is_public() {
        for model in &MODELS {
            assert_eq!(model.license.is_non_commercial(), model.gated, "{}", model.key);
            assert!(!model.languages.is_empty(), "{}", model.key);
            assert!(model.voices.contains(&"default"), "{}", model.key);
        }
        assert!(!model("phonon-31m-en").unwrap().gated);
        assert!(model("phonon-31m-en").unwrap().license == License::Apache20);
    }

    #[test]
    fn the_default_model_exists_and_pt_is_in_the_catalog() {
        assert_eq!(DEFAULT_MODEL, "phonon-40m-pt");
        assert!(model(DEFAULT_MODEL).is_some());
        assert_eq!(model("phonon-40m-pt").unwrap().languages, &["pt"]);
        assert!(model("inventado").is_none());
    }
}
