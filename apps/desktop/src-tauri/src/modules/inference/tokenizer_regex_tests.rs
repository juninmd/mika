//! Whisper e Supertonic carregam `tokenizer.json` com normalizador `Replace` por regex. Sem o
//! backend de regex compilado no crate `tokenizers`, os dois falham em runtime e a Mika não ouve
//! nem fala — e nenhum teste de unidade via isso, só o E2E de voz.

#[test]
fn a_tokenizer_with_regex_normalizer_and_split_loads() {
    let json = r#"{
      "version": "1.0",
      "truncation": null,
      "padding": null,
      "added_tokens": [],
      "normalizer": {"type": "Replace", "pattern": {"Regex": "[ ]+"}, "content": " "},
      "pre_tokenizer": {"type": "Split", "pattern": {"Regex": "[ ]+"}, "behavior": "Removed", "invert": false},
      "post_processor": null,
      "decoder": null,
      "model": {"type": "WordLevel", "vocab": {"[UNK]": 0, "oi": 1, "mika": 2}, "unk_token": "[UNK]"}
    }"#;
    let path = std::env::temp_dir().join(format!("mika-tokenizer-regex-{}.json", std::process::id()));
    std::fs::write(&path, json).unwrap();
    let canonical = tokenizers::canonicalize_file(&path);
    let _ = std::fs::remove_file(&path);
    let canonical = canonical.expect("tokenizer.json não converteu para o formato canônico");
    tokenizers::from_json(&canonical).expect("tokenizer com Replace/Split por regex tem de carregar");
}
