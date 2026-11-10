//! === packages/sdk/src/contracts/slang.ts::DEFAULT_SLANG_DICTIONARY ==============================
//!
//! Fallback quando `character.slangDictionary` (config do usuário) não tem NENHUMA entrada em
//! nenhum dos três modos — mesmo conteúdo do TS, termo a termo (ver `contracts/slang.ts`).

pub struct SlangEntry {
    pub term: &'static str,
    pub meaning: &'static str,
    pub register: &'static str,
}

pub const TECHNICAL: &[SlangEntry] = &[
    SlangEntry { term: "fiação exposta", meaning: "código fortemente acoplado, sem isolamento", register: "noir" },
    SlangEntry { term: "gambiarra de neon", meaning: "workaround feio mas funcional", register: "cyberpunk" },
    SlangEntry { term: "beco molhado", meaning: "caminho de código duvidoso, cheio de side-effects", register: "noir" },
    SlangEntry { term: "firewall furado", meaning: "validação ausente em borda de sistema", register: "cyberpunk" },
    SlangEntry { term: "cabo solto", meaning: "dependência implícita não declarada", register: "noir" },
];

pub const COMBAT: &[SlangEntry] = &[
    SlangEntry { term: "hackeado pelo mainframe", meaning: "tomou dano absurdo / morreu de forma humilhante", register: "cyberpunk" },
    SlangEntry { term: "cano queimado", meaning: "reload mal calculado, ficou sem munição na hora errada", register: "noir" },
    SlangEntry { term: "circuito frito", meaning: "erro tático grave, paralisou na hora do combate", register: "cyberpunk" },
    SlangEntry { term: "sombra na esquina", meaning: "emboscada óbvia que o usuário não viu chegar", register: "noir" },
    SlangEntry { term: "overclock burro", meaning: "agressividade sem leitura, levou hit desnecessário", register: "cyberpunk" },
];

pub const IDLE: &[SlangEntry] = &[
    SlangEntry { term: "rua deserta", meaning: "silêncio prolongado, contexto parado", register: "noir" },
    SlangEntry { term: "standby de neon", meaning: "esperando ação, energia contida", register: "cyberpunk" },
];
