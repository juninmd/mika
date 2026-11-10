//! Detector determinístico de crise na ENTRADA do usuário (autolesão/suicídio), nos 10 locales.
//!
//! Por que em código e não só no prompt: o piso de segurança de `system.md` é texto, e um modelo
//! local pequeno com `customInstructions`/persona "provocadora" por cima o ignora. Este módulo não
//! depende do modelo: o turno que trouxer um sinal explícito ganha uma nota de sistema "crise"
//! (`system_note`) e a resposta do `chat-send` carrega `crisisRegion` (`region`): o renderer mostra um
//! cartão discreto com os recursos do país.
//!
//! Régua: lista CONSERVADORA de frases em primeira pessoa ("quero me matar", "want to die"), casadas
//! por PALAVRA INTEIRA (nas bordas alfabéticas; CJK/hangul colam partículas, ali vale o prefixo). Três
//! camadas, nessa ordem:
//! 1. BENIGN — colocações FECHADAS ("morrer de rir", "kill myself laughing", "live in Texas") saem do
//!    texto por token inteiro. Nunca uma frase genérica ("me matar de", "quero morrer de"): ela apagava
//!    "vou me matar de verdade" e o positivo sumia. Na dúvida, a frase fica e o cartão gentil aparece.
//! 2. POSITIVE duro — dispara sempre ("end my life", "suicidal", "自殺したい").
//! 3. POSITIVE brando (`~`) — hipérbole comum de gamer/meme ("I want to die", "kms", "quero me
//!    matar"): só dispara SEM marcador de jogo/humor no texto (`CONTEXT`: "boss fight", "respawn",
//!    "lol", "kkk"…). Intenção clara sem marcador ("vou me matar depois da prova") dispara.
//!
//! Normalização: NFKC (fullwidth → ASCII), minúsculas, NFD sem marcas combinantes (acento, inclusive
//! o "não" decomposto do macOS) e sem zero-width, apóstrofos fora, espaço único. Sem rede, sem
//! telemetria, nada é gravado: o texto só é lido em memória.

use std::ops::Range;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

/// Frases que sinalizam crise. `{a|b}` expande em alternativas (aninhável; `{a |}` = opcional);
/// prefixo `~` = brando (suprimido por marcador de jogo/humor).
const POSITIVE: &[&str] = &[
    // en
    "~kill myself", "~killing myself", "end my life", "ending my life", "take my own life", "taking my own life",
    "~want to die", "~wanna die", "~wish i {was|were} dead", "wish i {was|were} never born",
    "~better off dead", "~better off without me", "{dont|do not} {want to|wanna} {live|be alive}",
    "no reason to live", "not worth living", "i am suicidal", "im suicidal", "feeling suicidal",
    "suicidal {thoughts|ideation}", "~commit suicide", "thinking {about|of} suicide", "considering suicide",
    "~unalive", "~kms", "~want to be dead", "~want to end it",
    "{want|wanna} to hurt myself", "going to hurt myself", "hurt myself on purpose",
    "harm myself", "harming myself", "cutting myself", "self-harm", "self harm",
    // pt-BR
    "~quero me matar", "~vou me matar", "pensando em me matar", "penso em me matar", "vontade de me matar",
    "~quero morrer", "~queria morrer", "vontade de morrer", "acabar com {a |}minha vida", "quero acabar com tudo",
    "tirar {a |}minha {propria |}vida", "nao quero {mais |}viver", "nao aguento mais viver", "nao quero {mais |}existir",
    "nao vale a pena viver", "queria nao ter nascido", "me suicidar", "pensando em suicid{io|ar}",
    "pensamentos suicidas", "ideacao suicida", "quero me machucar", "me machucar de proposito",
    "me cortar de proposito", "me cortando de proposito", "me automutilar", "automutilacao",
    "ninguem sentiria minha falta", "melhor se eu morresse",
    // es-ES
    "~quiero morir", "~quiero morirme", "~me quiero morir", "~quiero matarme", "~voy a matarme", "~me voy a matar",
    "~me quiero matar", "pensando en matarme", "ganas de {morir|morirme|matarme}", "quitarme la vida",
    "me quiero quitar la vida", "acabar con mi vida", "no quiero {vivir|seguir viviendo}",
    "no merece la pena vivir", "suicidarme", "me {quiero|voy a} suicidar", "quiero hacerme dano",
    "hacerme dano a proposito", "autolesion", "cortarme las venas", "nadie me echaria de menos",
    "pensamientos suicidas", "ideas suicidas",
    // fr-FR
    "~je veux mourir", "~envie de mourir", "me suicider", "~je veux me tuer", "~je vais me tuer", "~je vais en finir",
    "envie de me tuer", "mettre fin a mes jours", "en finir avec {la|ma} vie", "je ne veux plus vivre",
    "plus envie de vivre", "idees suicidaires", "pensees suicidaires", "envie de me faire du mal",
    "je veux me faire du mal", "automutilation", "me scarifier", "me couper les veines",
    "tout le monde serait mieux sans moi",
    // it-IT
    "~voglio morire", "~voglio uccidermi", "~voglio ammazzarmi", "voglia di morire", "togliermi la vita",
    "non voglio {piu vivere|vivere piu}", "non ho piu voglia di vivere", "pensieri suicidi", "penso al suicidio",
    "suicidarmi", "farla finita con {la vita|tutto}", "voglio farmi del male", "autolesionismo",
    "tagliarmi le vene", "starebbero meglio senza di me",
    // ja-JP
    "死にたい", "しにたい", "死にたくなる", "死のうと思", "自殺したい", "自殺しよう", "自殺を考え", "消えてしまいたい", "消えたい",
    "いなくなりたい", "生きていたくない", "生きたくない", "生きる意味がない", "自分を傷つけ", "リストカット", "自傷",
    "命を絶", "首を吊", "死んでしまいたい", "死んじゃいたい",
    // zh-CN (+ caracteres tradicionais comuns)
    "想死", "想去死", "不想活了", "不想活下去", "不想再活", "不想活着", "想自杀", "想自殺", "要自杀", "想要自杀", "自杀的念头",
    "自杀念头", "结束自己的生命", "结束生命", "活着没意思", "活着没有意义", "活不下去", "轻生", "輕生", "割腕",
    "自残", "自殘", "伤害自己", "死了算了",
    // ko-KR
    "죽고 싶", "죽고싶", "죽어버리고 싶", "죽어 버리고 싶", "죽고만 싶", "자살하고 싶", "자살하고싶", "자살할", "자살을 생각",
    "자살 생각", "자해", "목숨을 끊", "삶을 끝내", "살기 싫", "살고 싶지 않", "사라지고 싶",
    // ru-RU
    "~хочу умереть", "хочу покончить с собой", "покончить с собой", "покончить жизнь", "~хочу убить себя",
    "~убью себя", "не хочу {больше |}жить", "не хочется жить", "жить не хочу", "хочу сдохнуть", "суицидальн",
    "думаю о суициде", "думаю о самоубийстве", "свести счеты с жизнью", "порезать себе вены",
    "причинить себе вред", "навредить себе", "самоповреждени", "лучше бы меня не было",
    // de-DE
    "~ich will sterben", "~ich möchte sterben", "will nicht mehr leben", "keine lust mehr zu leben", "will tot sein",
    "~ich will mich {umbringen|töten}", "ich werde mich umbringen", "bringe mich um", "mich selbst umbringen",
    "mich selbst töten", "selbstmordgedanken", "selbstmord begehen", "suizid begehen", "an selbstmord denke",
    "an suizid denke", "suizidgedanken", "bin suizidal", "mir das leben nehmen", "mein leben beenden",
    "mich selbst verletzen", "will mir wehtun", "lieber tot", "besser ohne mich",
];

/// Colocações FECHADAS que contêm um positivo mas são expressão corriqueira: saem do texto (por
/// palavra inteira) antes da busca. Nada de "quero morrer de" solto — ver o cabeçalho.
const BENIGN: &[&str] = &[
    // en
    "{die|dying} {of|from} {laughter|laughing|embarrassment|boredom|shame|hunger|thirst|cold|heat|exhaustion|cringe}",
    "{die|dying} laughing", "{kill|killing} myself laughing", "killing myself with {my|the|your|a}",
    "cutting myself {some slack|a slice|a piece|a break|an apple|some cake}",
    "self{-| }harm {content|warning|warnings|themes|theme|tag|tags|trigger|triggers|scene|scenes|depiction|references|mentions|rating|awareness|prevention}",
    "{team|squad} {is|are|was|would be|will be} better off without me",
    "better off without me {in|on} {the|my|this} {team|squad|game|lobby|match}",
    "{dont|do not} {want to|wanna} live {here|there|alone|with {my parents|roommates|my roommate|him|her|them|my ex|my family}|in {texas|california|new york|london|the us|the usa|the uk|america|england|canada|australia|europe|germany|france|japan|brazil|china|russia|india|mexico|spain|italy|the suburbs|the city|a city|a big city|this city|this town|a small town|this house|this apartment|a dorm|my parents house|the country|the countryside}}",
    // pt-BR
    "{morrer|morrendo|morri|morria} de {rir|risada|vergonha|fome|sono|calor|frio|tedio|saudade|medo|cansaco|preguica|amor|raiva|curiosidade|inveja|ciume|pena|dor de cabeca}",
    "me matar de {estudar|trabalhar|rir|correr|malhar|treinar|tanto|fome|cansaco}",
    "me matar pra {terminar|entregar|chegar|passar|ganhar|conseguir|estudar|trabalhar}",
    "{vai|vao|ia|iam} acabar com {a |}minha vida", "{conteudo|aviso|gatilho|tema|temas|cena|cenas|tag} de automutilacao",
    "nao quero {mais |}viver {aqui|la|ai|sozinho|sozinha|com {meus pais|minha mae|meu pai|colegas}|em {sao paulo|portugal|lisboa|brasil|uma cidade|um apartamento|uma republica|uma casa|minas|curitiba|porto alegre|salvador|recife|fortaleza}}",
    // es-ES
    "{morir|morirme|matarme} de {risa|verguenza|hambre|sueno|calor|frio|aburrimiento|miedo|amor|envidia|cansancio|celos|pena|curiosidad|sed}",
    "{voy a matarme|me voy a matar|quiero matarme|me quiero matar} a {trabajar|estudiar|correr|entrenar|limpiar|cocinar|jugar|reir|hacer}",
    "{va|van|iba|iban} a acabar con mi vida",
    "no quiero {vivir|seguir viviendo} {aqui|alli|solo|sola|con {mis padres|mi madre|mi padre|companeros|mi pareja|nadie}|en {madrid|espana|barcelona|una ciudad|un piso|un apartamento|esta ciudad|este pueblo|mexico|argentina|la ciudad|un pueblo}}",
    // fr-FR
    "mourir de {rire|honte|faim|sommeil|chaud|froid|fatigue|peur|ennui|jalousie|curiosite|soif}",
    "je vais me tuer {au travail|a {travailler|etudier|courir|la tache|force de}}",
    "{je ne veux plus|plus envie de} vivre {ici|la|seul|seule|chez {moi|mes parents}|avec {mes parents|ma mere|mon pere|des colocataires}|en {ville|france}|a {paris|lyon|marseille|la campagne|la maison}}",
    // it-IT
    "morire {dal ridere|dalle risate|di {vergogna|fame|sonno|caldo|freddo|noia|paura|imbarazzo|stanchezza|sete|invidia|gelosia|curiosita}|dalla {vergogna|noia|fame|stanchezza|paura}}",
    "non voglio {piu vivere|vivere piu} {qui|li|da solo|da sola|con {i miei|i miei genitori|mia madre|mio padre|dei coinquilini}|in {citta|italia|una citta|un appartamento|questa citta|un paese}|a {roma|milano|napoli|torino|casa dei miei}}",
    // ja-JP
    "死ぬほど", "恥ずかしくて死にたい", "はずかしくて死にたい", "笑いすぎて死にたい",
    // zh-CN
    "想死你", "想死我", "想死他", "想死她", "想死大家", "想死记", "生命周期",
    // ko-KR
    "창피해서 죽고", "부끄러워서 죽고", "쪽팔려서 죽고",
    // ru-RU
    "умереть {от|со} {стыда|смеха|смеху|скуки|жары|голода|усталости|страха|любопытства|зависти|ревности|холода}",
    "не хочу жить {в|на} {москве|россии|этом городе|этой стране|этой квартире|этом доме|общежитии|деревне|питере|петербурге|пригороде|америке|германии}",
    "не хочу жить {здесь|тут|там|один|одна|с родителями|с мамой|с соседями|без {кофе|интернета|музыки|телефона|сна|еды}}",
    // de-DE
    "sterben vor {lachen|scham|hunger|langeweile|müdigkeit|neid|angst|durst|kälte|hitze}",
    "lieber tot als ohne {kaffee|internet|handy|wlan|musik|essen|schlaf}", "team ist besser ohne mich",
];

/// Marcadores de jogo/humor: com um deles no texto, os positivos brandos (`~`) não disparam. Só os
/// idiomas que têm positivo brando (CJK não tem: ali tudo é duro).
const CONTEXT: &[&str] = &[
    "boss fight", "boss battle", "final boss", "raid boss", "respawn", "in game", "in-game", "ingame", "grenade",
    "speedrun", "no jogo", "na partida", "en el juego", "en la partida", "dans le jeu", "dans la partie",
    "nel gioco", "в игре", "im spiel", "in der partie",
    "lol", "lmao", "lmfao", "rofl", "xd", "{haha|hahaha|hahahaha|jaja|jajaja|jajajaja|kkk|kkkk|kkkkk|kkkkkk|rsrs|mdr|ptdr|ahah|ahaha}",
    "хаха", "хахаха", "ахах", "лол", "кек",
];

struct Lists {
    hard: Vec<String>,
    soft: Vec<String>,
    benign: Vec<String>,
    context: Vec<String>,
}

/// `"a {b|c} d"` → `["a b d", "a c d"]`; grupos aninhados, alternativa vazia (`{a |}`) = opcional.
fn expand(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else { return vec![pattern.to_string()] };
    let mut depth = 0;
    let close = pattern[open..]
        .char_indices()
        .find_map(|(i, c)| {
            depth += i32::from(c == '{') - i32::from(c == '}');
            (depth == 0).then_some(open + i)
        })
        .expect("grupo `{` sem `}`");
    let (head, body, tail) = (&pattern[..open], &pattern[open + 1..close], &pattern[close + 1..]);
    let (mut alts, mut depth, mut from) = (Vec::new(), 0, 0);
    for (i, c) in body.char_indices() {
        depth += i32::from(c == '{') - i32::from(c == '}');
        if c == '|' && depth == 0 {
            alts.push(&body[from..i]);
            from = i + 1;
        }
    }
    alts.push(&body[from..]);
    alts.into_iter().flat_map(|alt| expand(&format!("{head}{alt}{tail}"))).collect()
}

fn lists() -> &'static Lists {
    static LISTS: OnceLock<Lists> = OnceLock::new();
    LISTS.get_or_init(|| {
        let build = |src: &[&str]| -> Vec<String> { src.iter().flat_map(|p| expand(p)).map(|p| fold(&p)).collect() };
        let (soft, hard): (Vec<&str>, Vec<&str>) = POSITIVE.iter().copied().partition(|p| p.starts_with('~'));
        let soft: Vec<&str> = soft.iter().map(|p| p.trim_start_matches('~')).collect();
        Lists { hard: build(&hard), soft: build(&soft), benign: build(BENIGN), context: build(CONTEXT) }
    })
}

/// NFKC (fullwidth → ASCII), minúsculas, NFD sem marcas combinantes (acento; `ё`→`е`; o "não"
/// decomposto do macOS) e sem zero-width, sem apóstrofo e com espaço único: o texto do usuário e as
/// frases passam pela MESMA dobra, então "don't"/"dont"/"Don’t" são uma só.
fn fold(text: &str) -> String {
    let decomposed = text
        .chars()
        .filter(|c| !matches!(c, '\'' | '’' | '‘' | '´' | '`' | 'ʼ'))
        .nfkc()
        .flat_map(char::to_lowercase)
        .nfd()
        .filter(|c| !matches!(c, '\u{0300}'..='\u{036F}' | '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}'))
        .nfc();
    let mut out = String::with_capacity(text.len());
    for ch in decomposed {
        match ch {
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Caractere de palavra: alfanumérico de escrita com espaço. CJK, kana e hangul colam partículas
/// ("死にたいです"), então não formam fronteira.
fn is_word(c: char) -> bool {
    c.is_alphanumeric()
        && !matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3040}'..='\u{30FF}' | '\u{3130}'..='\u{318F}' | '\u{3400}'..='\u{9FFF}' | '\u{AC00}'..='\u{D7AF}' | '\u{F900}'..='\u{FAFF}')
}

/// Ocorrências de `phrase` em `haystack` com fronteira de palavra no INÍCIO (qualquer escrita
/// alfabética) e no FIM (`whole_word`: idem; senão só ASCII — "суицидальн" e "want to die" ≠ "diet").
fn spans(haystack: &str, phrase: &str, whole_word: bool) -> Vec<Range<usize>> {
    let end_edge = |c: char| if whole_word { is_word(c) } else { c.is_ascii_alphanumeric() };
    let (first, last) = (phrase.chars().next(), phrase.chars().next_back());
    haystack
        .match_indices(phrase)
        .map(|(start, hit)| start..start + hit.len())
        .filter(|span| {
            let before = haystack[..span.start].chars().next_back();
            let after = haystack[span.end..].chars().next();
            !(first.is_some_and(is_word) && before.is_some_and(is_word)) && !(last.is_some_and(end_edge) && after.is_some_and(end_edge))
        })
        .collect()
}

/// Tira do texto as colocações benignas (por palavra inteira), deixando um espaço no lugar.
fn scrub(text: String, benign: &[String]) -> String {
    benign.iter().fold(text, |text, phrase| {
        let hits = spans(&text, phrase, true);
        let (mut out, mut at) = (String::with_capacity(text.len()), 0);
        for hit in &hits {
            out.push_str(&text[at..hit.start]);
            out.push(' ');
            at = hit.end;
        }
        out.push_str(&text[at..]);
        if hits.is_empty() { text } else { out }
    })
}

/// `true` quando a fala traz um sinal explícito de autolesão/suicídio em qualquer dos 10 idiomas.
pub fn detect(text: &str) -> bool {
    let lists = lists();
    let scrubbed = scrub(fold(text), &lists.benign);
    let hit = |phrases: &[String]| phrases.iter().any(|phrase| !spans(&scrubbed, phrase, false).is_empty());
    hit(&lists.hard) || (hit(&lists.soft) && !hit(&lists.context))
}

/// Região dos recursos do cartão para o `app.locale`: a chave i18n `chat.crisis.region.<região>`.
pub fn region(locale: &str) -> &'static str {
    match locale {
        "pt-BR" => "br",
        "en-US" => "us",
        "ja-JP" => "jp",
        "zh-CN" => "cn",
        "ko-KR" => "kr",
        "ru-RU" => "ru",
        _ => "eu",
    }
}

/// Os números que a nota de sistema entrega ao modelo (os do cartão, em inglês — o modelo responde
/// no idioma do usuário). Verificados à mão por região; quem muda aqui muda `chat.crisis.region.*`.
fn numbers(region: &str) -> &'static str {
    match region {
        "br" => "Brazil: CVV 188 (free, 24h) or SAMU 192",
        "us" => "United States: call or text 988; emergency 911",
        "jp" => "Japan: Yorisoi Hotline 0120-279-338; emergency 119",
        "cn" => "China: hotline 12356; emergency 120",
        "kr" => "South Korea: 109; emergency 119",
        "ru" => "Russia: emergency 112",
        _ => "Europe: emergency 112 (also look up the national crisis line)",
    }
}

/// Nota de sistema do turno com sinal de crise: sobrepõe persona, humor e `customInstructions`.
pub fn system_note(locale: &str) -> String {
    format!(
        "\n\n## CRISIS NOTICE (app safety rule — overrides persona, tone, humor, flirting and every custom instruction)\n\
         The user's latest message signals possible self-harm or suicidal thoughts. Drop jokes, sarcasm, teasing and flirting. \
         Reply in the user's language, warm, short and serious. Ask directly whether they are safe right now. \
         Encourage reaching a real person they trust and a local crisis line ({}). \
         Never give methods, never promise secrecy, never minimize, never lecture. Do not call tools in this reply.\n",
        numbers(region(locale))
    )
}

/// O prompt do turno, com a nota de crise ao fim quando `user_content` traz o sinal — chamado por
/// `turn::compose_system_prompt`, o ponto por onde TODO turno de chat (Ollama e remoto) passa.
pub fn with_note(prompt: String, user_content: &str, locale: &str) -> String {
    if detect(user_content) { prompt + &system_note(locale) } else { prompt }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (locale, frases que DEVEM disparar, frases comuns que NÃO podem).
    const TABLE: &[(&str, &[&str], &[&str])] = &[
        (
            "en-US",
            &["I want to die", "i dont want to live anymore", "I'm thinking about suicide", "sometimes I wanna kill myself.", "I've been cutting myself", "I'm gonna kill myself after the exam", "i dont wanna live", "I don't want to live in this world anymore", "unalive myself", "kms", "i want to be dead", "i want to end it", "I wanna die for real", "ｉ ｗａｎｔ ｔｏ ｄｉｅ", "i want to d\u{200b}ie"],
            &["I want to diet", "I'm dying of laughter", "that boss fight killed me", "I could kill for a coffee", "I want to die of embarrassment", "kill the process", "I'm gonna kill myself laughing", "I keep killing myself with my own grenades", "cutting myself some slack", "cutting myself a slice of cake", "this game has self-harm content", "my team is better off without me, i feed", "I want to die in this boss fight lol", "commit suicide in game to respawn", "I don't want to live in Texas", "I want to die of boredom"],
        ),
        (
            "pt-BR",
            &["quero me matar", "Eu não aguento mais viver", "tenho vontade de morrer", "to pensando em me suicidar", "queria não ter nascido", "vou me matar depois da prova", "vou me matar de verdade", "quero morrer de verdade", "não quero viver mais", "não quero viver", "vou acabar com minha vida", "quero tirar minha vida", "na\u{0303}o quero viver mais"],
            &["morri de rir", "quero morrer de vergonha", "vou me matar de estudar", "morrendo de fome", "esse bug me mata", "matar o processo travado", "vou me matar nesse boss kkk", "quero morrer de fome", "quero morrer de sono", "esse jogo tem aviso de automutilação", "não quero viver em são paulo", "esse chefe vai acabar com a minha vida"],
        ),
        (
            "es-ES",
            &["quiero morirme", "no quiero seguir viviendo", "tengo ganas de matarme", "he pensado en quitarme la vida", "me quiero morir", "quiero morir despacio", "me quiero matar", "me quiero suicidar", "quiero morirme de verdad"],
            &["me muero de risa", "me quiero morir de vergüenza", "voy a matarme a trabajar", "matar el proceso", "quiero morirme de risa", "quiero morirme en este jefe jaja", "quiero morir de hambre", "quiero morirme de risa jajaja"],
        ),
        (
            "fr-FR",
            &["je veux mourir", "j'ai envie de mourir", "je ne veux plus vivre", "je pense à me suicider", "j’ai des pensées suicidaires", "je vais me tuer aujourd'hui", "je veux mourir pour de vrai"],
            &["je meurs de rire", "je veux mourir de honte", "j'ai envie de mourir de rire", "il faut tuer le processus", "je vais me tuer au travail", "je veux mourir sur ce boss mdr", "je veux mourir de faim", "je vais me tuer a travailler"],
        ),
        (
            "it-IT",
            &["voglio morire", "non voglio più vivere", "ho voglia di morire", "voglio uccidermi", "a volte penso a togliermi la vita", "voglio morire davvero", "voglio morire stanotte"],
            &["muoio dal ridere", "voglio morire di vergogna", "voglio morire dal ridere", "uccidere il processo", "farla finita con le diete", "voglio morire contro questo boss ahah", "voglio morire di sonno"],
        ),
        (
            "ja-JP",
            &["もう死にたい", "消えてしまいたいです", "自殺したいと思っている", "リストカットをしてしまった", "生きていたくない", "消えたい", "死にたくなるほどつらい"],
            &["笑いすぎて死にたい", "死ぬほど笑った", "恥ずかしくて死にたい", "自殺点を取られた", "プロセスを終了する"],
        ),
        (
            "zh-CN",
            &["我想死", "我不想活了", "我想自杀", "最近总想轻生", "活着没意思", "我想去死", "想自殺"],
            &["笑死我了", "想死你了", "累死了", "我不想活动", "想死我了"],
        ),
        (
            "ko-KR",
            &["죽고 싶어", "자살하고 싶다", "살기 싫어요", "요즘 자해를 해요", "사라지고 싶어", "죽어버리고 싶다"],
            &["웃겨 죽겠다", "배고파 죽겠어", "창피해서 죽고 싶다", "프로세스를 종료해", "피곤해 죽겠네"],
        ),
        (
            "ru-RU",
            &["хочу умереть", "я не хочу жить", "думаю о суициде", "Хочу покончить с собой", "у меня суицидальные мысли", "не хочу больше жить", "я не хочу жить вообще"],
            &["умираю от смеха", "хочу умереть от стыда", "убить процесс", "я не хочу жить без кофе", "умер от скуки", "хочу умереть на этом боссе хаха", "не хочу жить без интернета"],
        ),
        (
            "de-DE",
            &["ich will sterben", "Ich will nicht mehr leben", "ich will mich umbringen", "ich habe Suizidgedanken", "ich will mir das Leben nehmen", "ich möchte sterben", "ich will sterben, wirklich"],
            &["ich sterbe vor lachen", "ich will sterben vor Scham", "den Prozess beenden", "ich bin lieber tot als ohne Kaffee", "tot gelacht", "ich will sterben bei diesem boss lol", "ich will sterben vor hunger"],
        ),
    ];

    #[test]
    fn detects_explicit_crisis_statements_in_all_ten_locales() {
        for (locale, positives, _) in TABLE {
            for text in *positives {
                assert!(detect(text), "[{locale}] deveria detectar: {text}");
            }
        }
    }

    #[test]
    fn common_idioms_and_tech_talk_are_not_crises() {
        for (locale, _, negatives) in TABLE {
            for text in *negatives {
                assert!(!detect(text), "[{locale}] falso positivo: {text}");
            }
        }
    }

    #[test]
    fn the_table_covers_every_locale_the_app_ships() {
        let locales: Vec<&str> = TABLE.iter().map(|(locale, ..)| *locale).collect();
        assert_eq!(locales, ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"]);
    }

    #[test]
    fn empty_and_ordinary_text_never_triggers() {
        assert!(!detect(""));
        assert!(!detect("   "));
        assert!(!detect("abre o spotify e toca algo calmo"));
    }

    #[test]
    fn every_locale_maps_to_a_region_whose_numbers_the_note_carries() {
        for (locale, ..) in TABLE {
            let note = system_note(locale);
            assert!(note.contains(numbers(region(locale))), "{locale}: {note}");
            assert!(note.contains("Do not call tools"));
        }
        assert_eq!(region("pt-BR"), "br");
        assert!(system_note("pt-BR").contains("188") && system_note("en-US").contains("988") && system_note("de-DE").contains("112"));
    }

    #[test]
    fn the_note_is_appended_only_to_turns_that_carry_the_signal() {
        let base = "prompt base".to_string();
        assert_eq!(with_note(base.clone(), "bora jogar", "pt-BR"), base);
        let flagged = with_note(base.clone(), "quero me matar", "pt-BR");
        assert!(flagged.starts_with("prompt base") && flagged.contains("CRISIS NOTICE") && flagged.contains("CVV 188"), "{flagged}");
    }
}
