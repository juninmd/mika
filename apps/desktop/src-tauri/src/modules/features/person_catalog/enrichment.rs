//! Enriquecimento automático — porte de
//! `apps/desktop/src/features/person-catalog/services/person-enrich.ts`.
//!
//! **As DUAS fontes do TS, na mesma ordem.** `fetchWikiSummary` (resumo da Wikipedia REST,
//! `GET https://en.wikipedia.org/api/rest_v1/page/summary/<slug>`) roda PRIMEIRO e é a fonte
//! primária de bio; a busca web (`browser::search`, motor "auto" — cadeia Bing→DuckDuckGo→Yahoo)
//! roda depois e só preenche o que a
//! Wikipedia deixou vazio — 404, timeout ou `extract` ausente/vazio caem no mesmo `None`, sem
//! distinção de causa (mesmo comportamento do TS: `catch { return null }`). A rede da Wikipedia
//! passa pelo mesmo pipeline SSRF/offline-guardado que a busca (`modules::browser::fetch_bytes`,
//! que por baixo é `guarded_fetch::guarded_get` — nunca um `reqwest::Client` novo).
//!
//! O restante do pipeline é fiel: mesma condição de preenchimento ("só se o campo já não existe no
//! registro"), mesmo `enriched: true` sempre no fim (decidido por `lifecycle.rs`/`mutate/enrich.rs`,
//! não aqui — dedup: `lifecycle.rs` só chama `enrich_person` na CRIAÇÃO da pessoa, nunca num
//! upsert repetido), mesmo silêncio em falha de rede (log + segue sem enriquecer, nunca propaga
//! erro pra quem chamou).

use std::time::Duration;

use serde::Deserialize;

use crate::config::store::ConfigStore;
use crate::modules::browser::{self, BrowserError, SearchResult};

use super::search_extract::{build_search_query, mentions_person, result_snippets, search_birth_date, search_birth_place, search_nationality};
use super::types::{PersonRecord, PersonType};

/// = `WIKI_API` em `person-enrich.ts`.
const WIKI_SUMMARY_API: &str = "https://en.wikipedia.org/api/rest_v1/page/summary";
/// = `TIMEOUT_MS` em `person-enrich.ts` — o `AbortController` interno, que sempre dispara antes do
/// timeout de 15s do `loggedFetch` (a guarda externa nunca chega a valer).
const WIKI_TIMEOUT: Duration = Duration::from_secs(6);
/// = `wiki.extract.slice(0, 600)` em `applyWikiPatch` (`person-enrich.ts`).
const WIKI_BIO_MAX_CHARS: usize = 600;

#[derive(Debug, Deserialize)]
struct WikiImage {
    source: String,
}

#[derive(Debug, Deserialize)]
struct WikiSummary {
    extract: Option<String>,
    thumbnail: Option<WikiImage>,
    originalimage: Option<WikiImage>,
    description: Option<String>,
    birth_date: Option<String>,
    birth_place: Option<String>,
    death_date: Option<String>,
}

/// Resultado interpretado da chamada à Wikipedia — nunca um erro que escapa pro chamador.
enum WikiOutcome {
    Found(WikiSummary),
    /// 404 — pessoa sem verbete, caso normal pra gente real. Fallback pra busca.
    NotFound,
    /// Timeout, erro de rede, SSRF, offline ou corpo que não parseia como `WikiSummary`. Mesmo
    /// destino do TS: `catch { return null }`, sem distinguir a causa.
    Unavailable,
}

/// = `encodeURIComponent(name.replace(/ /g, '_'))` em `fetchWikiSummary`. Nomes longos (até 255
/// chars, validado na entrada em `rpc.rs`) e unicode passam pelo mesmo `encode_uri_component` que
/// `browser::build_search_url` usa — sem tabela duplicada.
fn wiki_summary_url(name: &str) -> String {
    let slug = browser::encode_uri_component(&name.replace(' ', "_"));
    format!("{WIKI_SUMMARY_API}/{slug}")
}

/// Pura: interpreta o resultado já resolvido de [`browser::fetch_bytes`] (ou do timeout que o
/// envolve) sem tocar rede — é o que os testes deste arquivo exercitam.
fn interpret_wiki_response(result: Result<Vec<u8>, BrowserError>) -> WikiOutcome {
    match result {
        Ok(body) => serde_json::from_slice::<WikiSummary>(&body).map(WikiOutcome::Found).unwrap_or(WikiOutcome::Unavailable),
        Err(BrowserError::BadStatus(404)) => WikiOutcome::NotFound,
        Err(_) => WikiOutcome::Unavailable,
    }
}

/// = `!person.bio && wiki.extract` em `applyWikiPatch`, já com o truncamento de 600 chars. Pura.
fn wiki_bio(person_bio: Option<&str>, extract: Option<&str>) -> Option<String> {
    if person_bio.is_some() {
        return None;
    }
    let trimmed = extract?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(WIKI_BIO_MAX_CHARS).collect())
}

/// = `!person.<campo> && wiki.<campo>` em `applyWikiPatch` — genérico pros campos que só copiam o
/// texto, sem truncamento (`birthDate`/`birthPlace`/`deathDate`/`physicalDescription`). Bio e
/// imagem têm regra própria (`wiki_bio`/`wiki_image_url`).
fn fill_if_absent(existing: Option<&str>, candidate: Option<&str>) -> Option<String> {
    if existing.is_some() {
        return None;
    }
    candidate.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// SecOps: a URL de imagem nunca é buscada pelo servidor — só aceita o LINK, e só quando HTTPS e
/// host exatamente `upload.wikimedia.org` (qualquer path: `/wikipedia/<project>/...`). O JSON da
/// Wikipedia REST é resposta de rede como outra qualquer; um host diferente é descartado mesmo que
/// o formato pareça uma URL de imagem válida.
fn is_wikimedia_image_url(url: &str) -> bool {
    url::Url::parse(url).map(|parsed| parsed.scheme() == "https" && parsed.host_str() == Some("upload.wikimedia.org")).unwrap_or(false)
}

/// = as duas linhas de `applyWikiPatch` que preenchem `imageUrl` — `originalimage` vence quando os
/// dois existem (mesmo efeito do TS: o segundo `if` roda depois e sobrescreve o primeiro).
fn wiki_image_url(person_image_url: Option<&str>, wiki: &WikiSummary) -> Option<String> {
    if person_image_url.is_some() {
        return None;
    }
    let source = wiki.originalimage.as_ref().or(wiki.thumbnail.as_ref())?.source.as_str();
    is_wikimedia_image_url(source).then(|| source.to_string())
}

async fn fetch_wiki_summary(store: &ConfigStore, name: &str) -> WikiOutcome {
    let url = wiki_summary_url(name);
    match tokio::time::timeout(WIKI_TIMEOUT, browser::fetch_bytes(store, &url)).await {
        Ok(result) => interpret_wiki_response(result),
        Err(_) => WikiOutcome::Unavailable,
    }
}

/// = `ENRICHABLE_PERSON_TYPES` (`person-catalog-lifecycle.ts`).
const ENRICHABLE_PERSON_TYPES: &[PersonType] = &[
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
];

pub(super) fn is_enrichable(kind: PersonType) -> bool {
    ENRICHABLE_PERSON_TYPES.contains(&kind)
}

/// Patch resultante — sempre aplicado via `CatalogStore::upsert` (que já funde preservando campos
/// existentes quando o `UpsertInput` traz `None`, ver `mutate/upsert.rs::merge_upsert_into_existing`),
/// nunca uma escrita direta no registro.
#[derive(Debug, Default, Clone)]
pub(super) struct EnrichmentPatch {
    pub bio: Option<String>,
    pub birth_date: Option<String>,
    pub birth_place: Option<String>,
    pub death_date: Option<String>,
    pub physical_description: Option<String>,
    pub image_url: Option<String>,
    pub nationality: Option<String>,
    pub occupation: Option<Vec<String>>,
    pub known_for: Option<Vec<String>>,
    pub voice_actor: Option<String>,
}

/// Porte de `enrichPerson`. Nunca falha para o chamador: erro de rede/SSRF/offline (Wikipedia OU
/// busca) vira `log::warn!`/silêncio e um patch parcial (ou vazio), com `enriched: true` decidido
/// por `lifecycle.rs` (não aqui — este módulo só produz o patch de CAMPOS, quem marca `enriched` é
/// quem já sabe que a enriquização foi tentada).
pub(super) async fn enrich_person(store: &ConfigStore, person: &PersonRecord) -> EnrichmentPatch {
    let mut patch = EnrichmentPatch::default();

    if let WikiOutcome::Found(wiki) = fetch_wiki_summary(store, &person.primary_name).await {
        patch.bio = wiki_bio(person.bio.as_deref(), wiki.extract.as_deref());
        patch.image_url = wiki_image_url(person.image_url.as_deref(), &wiki);
        patch.birth_date = fill_if_absent(person.birth_date.as_deref(), wiki.birth_date.as_deref());
        patch.birth_place = fill_if_absent(person.birth_place.as_deref(), wiki.birth_place.as_deref());
        patch.death_date = fill_if_absent(person.death_date.as_deref(), wiki.death_date.as_deref());
        patch.physical_description = fill_if_absent(person.physical_description.as_deref(), wiki.description.as_deref());
    }

    let query = build_search_query(person);

    let results = match browser::search(store, &query, browser::AUTO_ENGINE).await {
        Ok(results) => results,
        Err(error) => {
            log::warn!("enriquecimento de pessoa via busca falhou, seguindo sem enriquecer: {error:?}");
            return patch;
        }
    };

    let relevant: Vec<&SearchResult> = results.iter().filter(|r| mentions_person(r, &person.primary_name)).take(4).collect();
    let snippets = result_snippets(&relevant);

    if patch.bio.is_none() && person.bio.is_none() {
        if let Some(first) = relevant.first().filter(|r| !r.snippet.trim().is_empty()) {
            patch.bio = Some(first.snippet.clone());
        }
    }
    if patch.birth_date.is_none() && person.birth_date.is_none() {
        patch.birth_date = search_birth_date(&snippets);
    }
    if patch.birth_place.is_none() && person.birth_place.is_none() {
        patch.birth_place = search_birth_place(&snippets);
    }
    if patch.nationality.is_none() && person.nationality.is_none() {
        patch.nationality = search_nationality(&snippets);
    }

    patch
}

#[cfg(test)]
mod tests;
