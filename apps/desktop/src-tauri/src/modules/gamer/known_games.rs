//! Port de `services/known-games.ts` — tabela estática de jogos não-Steam reconhecidos pelo nome
//! do processo (launchers Epic/Riot/Battle.net/Xbox/EA/Ubisoft/GOG e afins). Fielmente copiada:
//! remover uma entrada aqui é a companheira "esquecer" um jogo que o TS ainda reconhece.

use std::sync::OnceLock;

pub struct KnownGame {
    pub display_name: &'static str,
    pub process_names: &'static [&'static str],
}

pub static KNOWN_GAMES: &[KnownGame] = &[
    KnownGame { display_name: "Fortnite", process_names: &["fortnite"] },
    KnownGame { display_name: "Rocket League", process_names: &["rocketleague"] },
    KnownGame { display_name: "Fall Guys", process_names: &["fallguys"] },
    KnownGame { display_name: "Alan Wake 2", process_names: &["alanwake2"] },
    KnownGame { display_name: "League of Legends", process_names: &["leagueoflegends", "leagueclient"] },
    KnownGame { display_name: "Valorant", process_names: &["valorant"] },
    KnownGame { display_name: "Teamfight Tactics", process_names: &["tft"] },
    KnownGame { display_name: "Legends of Runeterra", process_names: &["runeterra"] },
    KnownGame { display_name: "Wild Rift", process_names: &["wildrift"] },
    KnownGame { display_name: "Call of Duty", process_names: &["cod"] },
    KnownGame { display_name: "Call of Duty: Warzone", process_names: &["warzone"] },
    KnownGame { display_name: "Call of Duty: Black Ops 6", process_names: &["blackops6", "bo6"] },
    KnownGame { display_name: "Overwatch 2", process_names: &["overwatch"] },
    KnownGame { display_name: "Diablo IV", process_names: &["diabloiv", "diablo4"] },
    KnownGame {
        display_name: "World of Warcraft",
        process_names: &["wow", "wowclassic", "wowretail", "worldofwarcraft"],
    },
    KnownGame { display_name: "Hearthstone", process_names: &["hearthstone"] },
    KnownGame { display_name: "StarCraft II", process_names: &["starcraft2", "sc2x64"] },
    KnownGame { display_name: "Halo Infinite", process_names: &["haloinfinite"] },
    KnownGame { display_name: "Halo: The Master Chief Collection", process_names: &["halomcc", "halo"] },
    KnownGame { display_name: "Forza Horizon 5", process_names: &["forzahorizon5", "forza"] },
    KnownGame { display_name: "Forza Motorsport", process_names: &["forzamotorsport"] },
    KnownGame { display_name: "Gears 5", process_names: &["gears5", "gears"] },
    KnownGame { display_name: "Sea of Thieves", process_names: &["seaofthieves"] },
    KnownGame { display_name: "Microsoft Flight Simulator", process_names: &["flightsimulator", "msfs"] },
    KnownGame { display_name: "Starfield", process_names: &["starfield"] },
    KnownGame { display_name: "EA Sports FC 25", process_names: &["eafc25", "fifa"] },
    KnownGame { display_name: "Apex Legends", process_names: &["apexlegends", "apex"] },
    KnownGame { display_name: "Battlefield", process_names: &["battlefield"] },
    KnownGame { display_name: "The Sims 4", process_names: &["sims4"] },
    KnownGame { display_name: "F1 24", process_names: &["f124"] },
    KnownGame { display_name: "Assassin's Creed Shadows", process_names: &["assassinscreed"] },
    KnownGame { display_name: "Rainbow Six Siege", process_names: &["rainbowsix", "r6siege"] },
    KnownGame { display_name: "Far Cry 6", process_names: &["farcry6"] },
    KnownGame { display_name: "The Division 2", process_names: &["division2"] },
    KnownGame { display_name: "Cyberpunk 2077", process_names: &["cyberpunk2077"] },
    KnownGame { display_name: "The Witcher 3", process_names: &["witcher3"] },
    KnownGame { display_name: "Minecraft", process_names: &["minecraft"] },
    KnownGame { display_name: "Grand Theft Auto V", process_names: &["gta5", "gtav"] },
    KnownGame { display_name: "Grand Theft Auto VI", process_names: &["gta6", "gtavi"] },
    KnownGame { display_name: "Red Dead Redemption 2", process_names: &["rdr2"] },
    KnownGame { display_name: "Elden Ring", process_names: &["eldenring"] },
    KnownGame { display_name: "Baldur's Gate 3", process_names: &["baldursgate3", "bg3"] },
    KnownGame { display_name: "Elden Ring Nightreign", process_names: &["nightreign"] },
    KnownGame { display_name: "Dark Souls III", process_names: &["darksouls3", "ds3"] },
    KnownGame { display_name: "Sekiro", process_names: &["sekiro"] },
    KnownGame { display_name: "Hades", process_names: &["hades"] },
    KnownGame { display_name: "Hades II", process_names: &["hades2"] },
    KnownGame { display_name: "Hollow Knight", process_names: &["hollowknight"] },
    KnownGame { display_name: "Silksong", process_names: &["silksong"] },
    KnownGame { display_name: "Stardew Valley", process_names: &["stardewvalley"] },
    KnownGame { display_name: "Terraria", process_names: &["terraria"] },
    KnownGame { display_name: "Factorio", process_names: &["factorio"] },
    KnownGame { display_name: "Satisfactory", process_names: &["satisfactory"] },
    KnownGame { display_name: "Subnautica", process_names: &["subnautica"] },
    KnownGame { display_name: "Palworld", process_names: &["palworld"] },
    KnownGame { display_name: "Helldivers 2", process_names: &["helldivers2"] },
    KnownGame { display_name: "Path of Exile 2", process_names: &["pathofexile2", "poe2"] },
    KnownGame { display_name: "Warframe", process_names: &["warframe"] },
    KnownGame { display_name: "Destiny 2", process_names: &["destiny2"] },
    KnownGame { display_name: "Counter-Strike 2", process_names: &["cs2"] },
    KnownGame { display_name: "Dota 2", process_names: &["dota2"] },
    KnownGame { display_name: "Team Fortress 2", process_names: &["tf2"] },
    KnownGame { display_name: "PUBG", process_names: &["pubg"] },
    KnownGame { display_name: "Dead by Daylight", process_names: &["deadbydaylight"] },
    KnownGame { display_name: "Phasmophobia", process_names: &["phasmophobia"] },
    KnownGame { display_name: "Lethal Company", process_names: &["lethalcompany"] },
    KnownGame { display_name: "Content Warning", process_names: &["contentwarning"] },
    KnownGame { display_name: "Valheim", process_names: &["valheim"] },
    KnownGame { display_name: "V Rising", process_names: &["vrising"] },
    KnownGame { display_name: "No Man's Sky", process_names: &["nomanssky"] },
    KnownGame { display_name: "Monster Hunter Wilds", process_names: &["monsterhunterwilds", "mhwilds"] },
    KnownGame { display_name: "Monster Hunter Rise", process_names: &["monsterhunterrise", "mhrise"] },
    KnownGame { display_name: "Street Fighter 6", process_names: &["streetfighter6"] },
    KnownGame { display_name: "Tekken 8", process_names: &["tekken8"] },
    KnownGame { display_name: "Dragon Ball: Sparking! Zero", process_names: &["sparkingzero"] },
    KnownGame { display_name: "Black Myth: Wukong", process_names: &["wukong"] },
    KnownGame { display_name: "Stellar Blade", process_names: &["stellarblade"] },
    KnownGame { display_name: "Returnal", process_names: &["returnal"] },
    KnownGame { display_name: "Ghost of Tsushima", process_names: &["ghostoftsushima"] },
    KnownGame { display_name: "God of War", process_names: &["godofwar"] },
    KnownGame { display_name: "God of War Ragnarök", process_names: &["gowragnarok", "ragnarok"] },
    KnownGame { display_name: "The Last of Us Part I", process_names: &["lastofus"] },
    KnownGame { display_name: "Horizon Forbidden West", process_names: &["horizonforbiddenwest"] },
    KnownGame { display_name: "Spider-Man Remastered", process_names: &["spiderman"] },
    KnownGame { display_name: "Spider-Man 2", process_names: &["spiderman2"] },
    KnownGame { display_name: "Days Gone", process_names: &["daysgone"] },
    KnownGame { display_name: "Death Stranding", process_names: &["deathstranding"] },
    KnownGame { display_name: "Disco Elysium", process_names: &["discoelysium"] },
    KnownGame { display_name: "Persona 5 Royal", process_names: &["persona5"] },
    KnownGame { display_name: "Persona 3 Reload", process_names: &["persona3"] },
    KnownGame { display_name: "Final Fantasy VII Remake", process_names: &["finalfantasy7", "ff7r"] },
    KnownGame { display_name: "Final Fantasy XVI", process_names: &["finalfantasy16", "ff16"] },
    KnownGame { display_name: "Like a Dragon: Infinite Wealth", process_names: &["likeadragon", "yakuza"] },
    KnownGame { display_name: "Stray", process_names: &["stray"] },
    KnownGame { display_name: "Cult of the Lamb", process_names: &["cultofthelamb"] },
    KnownGame { display_name: "Dave the Diver", process_names: &["davethediver"] },
    KnownGame { display_name: "Celeste", process_names: &["celeste"] },
    KnownGame { display_name: "Dead Cells", process_names: &["deadcells"] },
    KnownGame { display_name: "Blasphemous", process_names: &["blasphemous"] },
    KnownGame { display_name: "Slay the Spire", process_names: &["slaythespire"] },
    KnownGame { display_name: "Balatro", process_names: &["balatro"] },
    KnownGame { display_name: "Risk of Rain 2", process_names: &["riskofrain2"] },
    KnownGame { display_name: "Core Keeper", process_names: &["corekeeper"] },
    KnownGame { display_name: "Enshrouded", process_names: &["enshrouded"] },
    KnownGame { display_name: "Nightingale", process_names: &["nightingale"] },
    KnownGame { display_name: "Smalland", process_names: &["smalland"] },
    KnownGame { display_name: "Grounded", process_names: &["grounded"] },
    KnownGame { display_name: "Project Zomboid", process_names: &["projectzomboid"] },
    KnownGame { display_name: "7 Days to Die", process_names: &["7daystodie"] },
    KnownGame { display_name: "Euro Truck Simulator 2", process_names: &["eurotrucksimulator2", "ets2"] },
    KnownGame { display_name: "American Truck Simulator", process_names: &["americantrucksimulator", "ats"] },
    KnownGame { display_name: "Cities: Skylines II", process_names: &["citiesskylines"] },
    KnownGame { display_name: "Planet Coaster 2", process_names: &["planetcoaster2"] },
    KnownGame { display_name: "Total War: Warhammer III", process_names: &["totalwarwarhammer3", "warhammer3"] },
    KnownGame { display_name: "Age of Empires IV", process_names: &["ageofempires4"] },
    KnownGame { display_name: "Civilization VII", process_names: &["civ7", "civilization7"] },
    KnownGame { display_name: "Victoria 3", process_names: &["victoria3"] },
    KnownGame { display_name: "Hearts of Iron IV", process_names: &["hoi4"] },
    KnownGame { display_name: "Europa Universalis IV", process_names: &["eu4"] },
    KnownGame { display_name: "Crusader Kings III", process_names: &["crusaderkings3", "ck3"] },
    KnownGame { display_name: "Avowed", process_names: &["avowed"] },
    KnownGame { display_name: "Indiana Jones and the Great Circle", process_names: &["indianajones"] },
    KnownGame { display_name: "Metaphor: ReFantazio", process_names: &["metaphor", "metaphorrefantazio"] },
    KnownGame { display_name: "Dragon's Dogma 2", process_names: &["dragonsdogma2", "dd2"] },
    KnownGame { display_name: "Kingdom Come: Deliverance II", process_names: &["kingdomcome2", "kcd2"] },
    KnownGame { display_name: "Silent Hill 2", process_names: &["silenthill2", "sh2"] },
    KnownGame { display_name: "Warhammer 40,000: Space Marine 2", process_names: &["spacemarine2"] },
    KnownGame { display_name: "Final Fantasy VII Rebirth", process_names: &["ff7rebirth", "finalfantasy7rebirth"] },
    KnownGame { display_name: "Ghost of Yōtei", process_names: &["ghostofyotei"] },
    KnownGame { display_name: "Clair Obscur: Expedition 33", process_names: &["expedition33"] },
    KnownGame { display_name: "Marvel Rivals", process_names: &["marvelrivals"] },
    KnownGame { display_name: "Delta Force", process_names: &["deltaforce"] },
    KnownGame { display_name: "S.T.A.L.K.E.R. 2: Heart of Chornobyl", process_names: &["stalker2"] },
    KnownGame { display_name: "Manor Lords", process_names: &["manorlords"] },
    KnownGame { display_name: "Nine Sols", process_names: &["ninesols"] },
    KnownGame { display_name: "Animal Well", process_names: &["animalwell"] },
    KnownGame { display_name: "Zenless Zone Zero", process_names: &["zenlesszonezero", "zzz"] },
    KnownGame { display_name: "Genshin Impact", process_names: &["genshinimpact", "genshin"] },
    KnownGame { display_name: "Honkai: Star Rail", process_names: &["starrail", "honkaistarrail"] },
    KnownGame { display_name: "Wuthering Waves", process_names: &["wutheringwaves"] },
];

pub fn normalize_game_key(value: &str) -> String {
    value.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

type GameKeyIndex = Vec<(&'static KnownGame, Vec<String>)>;

/// Normalizado uma vez, não por chamada — mesma razão do comentário original: o watcher chamaria
/// isto por processo em execução, a cada tick.
static GAME_KEY_INDEX: OnceLock<GameKeyIndex> = OnceLock::new();

fn game_key_index() -> &'static GameKeyIndex {
    GAME_KEY_INDEX.get_or_init(|| {
        KNOWN_GAMES
            .iter()
            .map(|game| (game, game.process_names.iter().map(|name| normalize_game_key(name)).collect()))
            .collect()
    })
}

/// Casa pela chave MAIS ESPECÍFICA (mais longa), não pela ordem de declaração — senão
/// `"hades2".contains("hades")` atribuiria a sessão errada.
pub fn find_known_game(process_key: &str) -> Option<&'static KnownGame> {
    let mut best: Option<(&'static KnownGame, usize)> = None;
    for (game, keys) in game_key_index() {
        for key in keys {
            let matches = if key.len() >= 4 { process_key.starts_with(key.as_str()) } else { process_key == key.as_str() };
            if matches && best.is_none_or(|(_, len)| key.len() > len) {
                best = Some((game, key.len()));
            }
        }
    }
    best.map(|(game, _)| game)
}

/// Variante de [`find_known_game`] para casar contra o TÍTULO da janela em foco, não contra um
/// nome de processo — chamada só por `game_detection::find_foreground_game`. `process_names` são
/// mnemônicos curtos ("stray", "prey", "control"), e vários coincidem com palavras comuns do
/// inglês: um `.contains()` cru no título inteiro (o que [`find_known_game`] faz, correto para um
/// nome de PROCESSO, que não tem espaço) deu falso positivo real em produção — "Stray" batendo com
/// o Windows Terminal em foco (smoke-baseline.md item 8). Aqui a chave só bate por PALAVRA inteira
/// do título (`tokens`) ou, sendo longa o bastante (ver `game_detection::MIN_TITLE_BLOB_KEY_LEN`),
/// por substring do blob normalizado — mesma disciplina que `game_detection::title_matches_key` já
/// aplica ao catálogo Steam, agora também para este catálogo.
pub fn find_known_game_by_title(tokens: &std::collections::HashSet<String>, title_blob: &str) -> Option<&'static KnownGame> {
    const MIN_BLOB_KEY_LEN: usize = 8;
    let mut best: Option<(&'static KnownGame, usize)> = None;
    for (game, keys) in game_key_index() {
        for key in keys {
            let matches = tokens.contains(key) || (key.len() >= MIN_BLOB_KEY_LEN && title_blob.contains(key.as_str()));
            if matches && best.is_none_or(|(_, len)| key.len() > len) {
                best = Some((game, key.len()));
            }
        }
    }
    best.map(|(game, _)| game)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_game_by_exact_short_process_key() {
        let found = find_known_game("cs2").unwrap();
        assert_eq!(found.display_name, "Counter-Strike 2");
    }

    #[test]
    fn matches_the_most_specific_key_not_declaration_order() {
        // "hades2" contém "hades", mas o processo real de Hades II normaliza para "hades2".
        let found = find_known_game("hades2").unwrap();
        assert_eq!(found.display_name, "Hades II");
    }

    /// Defeito real: `securityhealthsystray` (bandeja do Windows) contém "stray" no meio e virava
    /// "Jogando Stray". Executável de jogo carrega a chave no INÍCIO (`stray-win64-shipping`).
    #[test]
    fn a_key_buried_inside_an_unrelated_process_name_does_not_match() {
        assert!(find_known_game("securityhealthsystray").is_none());
        assert_eq!(find_known_game("straywin64shipping").unwrap().display_name, "Stray");
    }

    #[test]
    fn unknown_process_returns_none() {
        assert!(find_known_game("notepad").is_none());
    }

    #[test]
    fn normalize_game_key_lowercases_and_strips_non_alphanumeric() {
        assert_eq!(normalize_game_key("Counter-Strike 2"), "counterstrike2");
    }

    fn tokens(title: &str) -> std::collections::HashSet<String> {
        title.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).map(|t| t.to_lowercase()).collect()
    }

    /// Reprodução direta do defeito real (smoke-baseline.md item 8): "stray" é uma chave curta de
    /// `known_games` e aparece como substring de "astray" — um blob cru (`.contains()`) bateria; a
    /// disciplina de palavra não deve.
    #[test]
    fn find_known_game_by_title_ignores_a_short_key_embedded_inside_a_longer_word() {
        let title = "Astray Thoughts Journal - Notepad2";
        let blob = normalize_game_key(title);
        assert!(find_known_game_by_title(&tokens(title), &blob).is_none());
    }

    #[test]
    fn find_known_game_by_title_matches_a_short_key_as_a_whole_word() {
        let title = "Stray - Steam";
        let blob = normalize_game_key(title);
        let found = find_known_game_by_title(&tokens(title), &blob).unwrap();
        assert_eq!(found.display_name, "Stray");
    }

    #[test]
    fn find_known_game_by_title_still_matches_a_long_compound_key_across_multiple_words() {
        let title = "Euro Truck Simulator 2 - now driving";
        let blob = normalize_game_key(title);
        let found = find_known_game_by_title(&tokens(title), &blob).unwrap();
        assert_eq!(found.display_name, "Euro Truck Simulator 2");
    }
}
