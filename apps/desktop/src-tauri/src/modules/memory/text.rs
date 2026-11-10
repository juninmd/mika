//! `stripDiacritics` compartilhado — porte de `packages/sdk/src/utils/text-normalize.ts`, usado
//! tanto por `episodic::search` (`normalizeMemoryText`) quanto por `social::rules`
//! (`normalizeText`), exatamente como o TS importa a MESMA função dos dois lados.
//!
//! **Divergência aceita, não nova**: o original usa `text.normalize('NFD').replace(/\p{Diacritic}/gu, '')`
//! — decomposição Unicode completa, cobre acento de QUALQUER script (vietnamita, grego, cirílico).
//! `unicode-normalization` não está em `Cargo.toml` e esta frente não pode declarar dependência nova
//! (ver `AGENTS.md` da tarefa). O mapa de caracteres abaixo é o MESMO padrão já aceito em
//! `modules/brain/guards.rs::strip_diacritics` (cobre pt-BR e línguas românicas vizinhas) — reaproveitado
//! aqui em vez de duplicado, para as duas frentes nunca divergirem por acidente. Um acento fora dessa
//! tabela (ex. "Hà Nội" em vietnamita) sobrevive ao normalizador — mesma lacuna, mesmo motivo.

/// Cobre os acentos latinos comuns (pt-BR e vizinhos). Ver divergência no cabeçalho do módulo.
pub fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ý' | 'ÿ' => 'y',
            'Ý' => 'Y',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_pt_br_accents() {
        assert_eq!(strip_diacritics("São Paulo é ótimo"), "Sao Paulo e otimo");
    }

    #[test]
    fn leaves_non_accented_and_non_latin_untouched() {
        assert_eq!(strip_diacritics("hello 世界"), "hello 世界");
    }
}
