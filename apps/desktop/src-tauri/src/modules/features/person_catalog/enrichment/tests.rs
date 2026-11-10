use super::*;

#[test]
fn every_expected_type_is_enrichable() {
    for kind in [
        PersonType::RealPerson,
        PersonType::FictionalCharacter,
        PersonType::Singer,
        PersonType::Streamer,
        PersonType::Vtuber,
        PersonType::Historical,
        PersonType::Author,
        PersonType::Comedian,
        PersonType::Game,
        PersonType::Series,
        PersonType::Movie,
        PersonType::Anime,
    ] {
        assert!(is_enrichable(kind), "{kind:?} devia ser enriquecível");
    }
}

#[test]
fn non_enrichable_types_are_excluded() {
    assert!(!is_enrichable(PersonType::Pet));
    assert!(!is_enrichable(PersonType::Organization));
    assert!(!is_enrichable(PersonType::Other));
}

// `build_search_query`/`mentions_person`/extração de nacionalidade/data/local de nascimento
// moraram aqui antes desta rodada — agora vivem (com seus próprios testes) em
// `search_extract.rs`/`search_extract/tests.rs`, único dono do parsing dos trechos de busca.

// --- wiki_summary_url: encoding (espaço, unicode, caracteres especiais) --------------------

#[test]
fn wiki_summary_url_replaces_spaces_with_underscore_before_encoding() {
    assert_eq!(wiki_summary_url("Ada Lovelace"), "https://en.wikipedia.org/api/rest_v1/page/summary/Ada_Lovelace");
}

#[test]
fn wiki_summary_url_percent_encodes_unicode_and_special_characters() {
    // Acento (unicode) e barra (reservado, precisa virar %2F para não quebrar o path).
    assert_eq!(
        wiki_summary_url("José Mourinho/Treinador"),
        "https://en.wikipedia.org/api/rest_v1/page/summary/Jos%C3%A9_Mourinho%2FTreinador"
    );
}

#[test]
fn wiki_summary_url_handles_a_255_char_name_without_panicking() {
    let long_name = "A".repeat(255);
    let url = wiki_summary_url(&long_name);
    assert!(url.starts_with(WIKI_SUMMARY_API));
    assert!(url.ends_with(&long_name));
}

// --- interpret_wiki_response: parsing e fallback (404 / erro / JSON inválido) ---------------

#[test]
fn interpret_wiki_response_parses_the_extract_from_fixture_json() {
    let fixture = r#"{"title":"Ada Lovelace","extract":"cientista britânica"}"#.as_bytes().to_vec();
    match interpret_wiki_response(Ok(fixture)) {
        WikiOutcome::Found(wiki) => assert_eq!(wiki.extract.as_deref(), Some("cientista britânica")),
        _ => panic!("esperava WikiOutcome::Found"),
    }
}

#[test]
fn interpret_wiki_response_falls_back_to_not_found_on_404() {
    assert!(matches!(interpret_wiki_response(Err(BrowserError::BadStatus(404))), WikiOutcome::NotFound));
}

#[test]
fn interpret_wiki_response_falls_back_to_unavailable_on_any_other_error() {
    assert!(matches!(interpret_wiki_response(Err(BrowserError::Network("timeout".into()))), WikiOutcome::Unavailable));
    assert!(matches!(interpret_wiki_response(Err(BrowserError::BadStatus(500))), WikiOutcome::Unavailable));
}

#[test]
fn interpret_wiki_response_falls_back_to_unavailable_on_invalid_json() {
    assert!(matches!(interpret_wiki_response(Ok(b"not json".to_vec())), WikiOutcome::Unavailable));
}

// --- wiki_bio: preenche só quando a pessoa ainda não tem bio, trunca em 600 chars -----------

#[test]
fn wiki_bio_fills_when_person_has_no_bio() {
    assert_eq!(wiki_bio(None, Some("Elden Ring is a 2022 action RPG.")), Some("Elden Ring is a 2022 action RPG.".to_string()));
}

#[test]
fn wiki_bio_never_overwrites_an_existing_bio() {
    assert_eq!(wiki_bio(Some("bio já existente"), Some("resumo da wiki")), None);
}

#[test]
fn wiki_bio_skips_a_missing_or_empty_extract() {
    assert_eq!(wiki_bio(None, None), None);
    assert_eq!(wiki_bio(None, Some("   ")), None);
}

#[test]
fn wiki_bio_truncates_at_600_chars() {
    let long_extract = "a".repeat(700);
    let bio = wiki_bio(None, Some(&long_extract)).expect("bio preenchida");
    assert_eq!(bio.chars().count(), WIKI_BIO_MAX_CHARS);
}

// --- fill_if_absent: birthDate/birthPlace/deathDate/physicalDescription -----------------

#[test]
fn fill_if_absent_fills_when_the_person_has_nothing() {
    assert_eq!(fill_if_absent(None, Some("1852-11-27")), Some("1852-11-27".to_string()));
}

#[test]
fn fill_if_absent_never_overwrites_an_existing_value() {
    assert_eq!(fill_if_absent(Some("já preenchido"), Some("da wiki")), None);
}

#[test]
fn fill_if_absent_skips_missing_or_blank_candidates() {
    assert_eq!(fill_if_absent(None, None), None);
    assert_eq!(fill_if_absent(None, Some("   ")), None);
}

// --- is_wikimedia_image_url / wiki_image_url: SecOps — só link, só upload.wikimedia.org -----

#[test]
fn is_wikimedia_image_url_accepts_the_expected_host() {
    assert!(is_wikimedia_image_url("https://upload.wikimedia.org/wikipedia/commons/a/a1/Ada.jpg"));
}

#[test]
fn is_wikimedia_image_url_rejects_http_and_other_hosts() {
    assert!(!is_wikimedia_image_url("http://upload.wikimedia.org/wikipedia/commons/a/a1/Ada.jpg"), "http não é https");
    assert!(!is_wikimedia_image_url("https://evil.example.com/upload.wikimedia.org/a.jpg"), "host errado, mesmo com o texto no path");
    assert!(!is_wikimedia_image_url("https://upload.wikimedia.org.evil.com/a.jpg"), "sufixo do host não é o host");
    assert!(!is_wikimedia_image_url("not a url"));
}

fn wiki_with_images(thumbnail: Option<&str>, originalimage: Option<&str>) -> WikiSummary {
    WikiSummary {
        extract: None,
        thumbnail: thumbnail.map(|s| WikiImage { source: s.to_string() }),
        originalimage: originalimage.map(|s| WikiImage { source: s.to_string() }),
        description: None,
        birth_date: None,
        birth_place: None,
        death_date: None,
    }
}

#[test]
fn wiki_image_url_prefers_originalimage_over_thumbnail() {
    let wiki = wiki_with_images(
        Some("https://upload.wikimedia.org/wikipedia/commons/thumb/a1/Ada.jpg"),
        Some("https://upload.wikimedia.org/wikipedia/commons/a1/Ada.jpg"),
    );
    assert_eq!(wiki_image_url(None, &wiki), Some("https://upload.wikimedia.org/wikipedia/commons/a1/Ada.jpg".to_string()));
}

#[test]
fn wiki_image_url_falls_back_to_thumbnail_when_no_originalimage() {
    let wiki = wiki_with_images(Some("https://upload.wikimedia.org/wikipedia/commons/thumb/a1/Ada.jpg"), None);
    assert_eq!(wiki_image_url(None, &wiki), Some("https://upload.wikimedia.org/wikipedia/commons/thumb/a1/Ada.jpg".to_string()));
}

#[test]
fn wiki_image_url_rejects_a_non_wikimedia_source_even_if_present() {
    let wiki = wiki_with_images(None, Some("https://attacker.example.com/fake.jpg"));
    assert_eq!(wiki_image_url(None, &wiki), None);
}

#[test]
fn wiki_image_url_never_overwrites_an_existing_image() {
    let wiki = wiki_with_images(None, Some("https://upload.wikimedia.org/wikipedia/commons/a1/Ada.jpg"));
    assert_eq!(wiki_image_url(Some("https://existing.example.com/a.jpg"), &wiki), None);
}

#[test]
fn interpret_wiki_response_parses_image_death_date_and_description_from_fixture() {
    let fixture = r#"{
        "title": "Ada Lovelace",
        "extract": "cientista britânica",
        "description": "matemática e escritora",
        "thumbnail": { "source": "https://upload.wikimedia.org/wikipedia/commons/thumb/a1/Ada.jpg", "width": 100, "height": 100 },
        "originalimage": { "source": "https://upload.wikimedia.org/wikipedia/commons/a1/Ada.jpg", "width": 400, "height": 400 },
        "death_date": "1852-11-27"
    }"#
    .as_bytes()
    .to_vec();
    match interpret_wiki_response(Ok(fixture)) {
        WikiOutcome::Found(wiki) => {
            assert_eq!(wiki.description.as_deref(), Some("matemática e escritora"));
            assert_eq!(wiki.death_date.as_deref(), Some("1852-11-27"));
            assert_eq!(wiki.originalimage.map(|i| i.source), Some("https://upload.wikimedia.org/wikipedia/commons/a1/Ada.jpg".to_string()));
        }
        _ => panic!("esperava WikiOutcome::Found"),
    }
}
