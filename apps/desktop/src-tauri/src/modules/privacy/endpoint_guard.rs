//! Guarda de SSRF para endpoint DIGITADO pelo usuário (LLM Ollama/OpenAI-compatible, TTS OpenAI) — porte de
//! `assertLLMBaseUrlAllowed` + `SecurityService.validateUrl` do TS. Loopback passa; o resto só com
//! http(s) e IP público — para nome de host, TODOS os endereços resolvidos têm de ser públicos, e DNS
//! que falha bloqueia (fail-closed). Depois, o gate de modo offline: são perguntas independentes.
//!
//! Esta checagem resolve e depois o cliente conecta, então sozinha ela não cobre DNS rebinding entre as duas
//! etapas; o pipeline de navegação (`browser::guarded_fetch`) fecha isso com `PublicOnlyResolver`, que filtra
//! o DNS na hora da conexão. A tabela de IPs bloqueados é a MESMA (`browser::net_guard`).

use std::net::{IpAddr, ToSocketAddrs};

use url::Host;

use crate::config::store::ConfigStore;
use crate::modules::browser::{is_blocked_ip, validate_navigable_url};

pub fn assert_user_endpoint_allowed(store: &ConfigStore, url: &str, provider_tag: &str) -> Result<(), String> {
    assert_endpoint_with(url, provider_tag, resolve_host)?;
    super::assert_network_allowed(store, url, provider_tag)
}

fn resolve_host(host: &str, port: u16) -> Option<Vec<IpAddr>> {
    (host, port).to_socket_addrs().ok().map(|addrs| addrs.map(|addr| addr.ip()).collect())
}

fn assert_endpoint_with(
    url: &str,
    provider_tag: &str,
    resolve: impl Fn(&str, u16) -> Option<Vec<IpAddr>>,
) -> Result<(), String> {
    url::Url::parse(url).map_err(|_| format!("{provider_tag}: endpoint inválido"))?;
    if super::is_loopback_url(url) {
        return Ok(());
    }
    let blocked = |why: &str| {
        log::warn!("endpoint configurado bloqueado por segurança (provider={provider_tag})");
        format!("{provider_tag}: endpoint bloqueado (SSRF: {why})")
    };
    let parsed = validate_navigable_url(url).map_err(|error| blocked(&error))?;
    let Some(Host::Domain(domain)) = parsed.host() else {
        return Ok(());
    };
    let port = parsed.port_or_known_default().unwrap_or(443);
    let ips = resolve(domain, port).filter(|ips| !ips.is_empty()).ok_or_else(|| blocked("host não resolve"))?;
    if ips.iter().any(|ip| is_blocked_ip(*ip)) {
        return Err(blocked("host resolve para IP privado/local"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolves_to(ips: &'static [&'static str]) -> impl Fn(&str, u16) -> Option<Vec<IpAddr>> {
        move |_, _| Some(ips.iter().map(|ip| ip.parse().unwrap()).collect())
    }

    fn unresolvable(_: &str, _: u16) -> Option<Vec<IpAddr>> {
        None
    }

    #[test]
    fn loopback_passes_without_touching_dns() {
        for url in ["http://localhost:11434", "http://127.0.0.1:8089/v1", "http://foo.localhost:1", "http://[::1]:1"] {
            assert!(assert_endpoint_with(url, "t", unresolvable).is_ok(), "{url}");
        }
    }

    /// O que o port loopback-only negava e o TS aceitava: provedor remoto com IP/host público.
    #[test]
    fn public_ip_or_publicly_resolving_host_passes() {
        assert!(assert_endpoint_with("https://8.8.8.8/v1", "t", unresolvable).is_ok());
        assert!(assert_endpoint_with("https://api.groq.com/openai/v1", "t", resolves_to(&["104.18.2.1"])).is_ok());
    }

    #[test]
    fn private_link_local_and_metadata_literals_are_blocked() {
        for url in ["http://192.168.1.50:11434", "http://10.0.0.1", "http://172.16.0.1", "http://169.254.169.254/latest", "http://[fd00::1]"] {
            let err = assert_endpoint_with(url, "t", unresolvable).unwrap_err();
            assert!(err.contains("SSRF"), "{url}: {err}");
        }
    }

    /// Nome público que aponta para a rede interna é o SSRF clássico: um endereço privado já bloqueia.
    #[test]
    fn host_resolving_to_any_private_address_is_blocked() {
        let err = assert_endpoint_with("https://evil.example", "t", resolves_to(&["93.184.216.34", "10.0.0.5"])).unwrap_err();
        assert!(err.contains("privado"), "{err}");
    }

    /// Mesma tabela do `browser` (uma fonte só): CGNAT, benchmarking, NAT64 com IPv4 embutido e nome
    /// `.internal`/`.local` valem para o endpoint digitado também.
    #[test]
    fn the_shared_table_also_guards_typed_endpoints() {
        static BLOCKED: [&str; 6] = ["100.64.0.1", "198.18.0.1", "0.0.0.0", "224.0.0.1", "64:ff9b::a00:1", "ff02::1"];
        for ip in &BLOCKED {
            let err = assert_endpoint_with("https://evil.example", "t", resolves_to(std::slice::from_ref(ip))).unwrap_err();
            assert!(err.contains("privado"), "{ip}: {err}");
        }
        for url in ["https://db.internal/v1", "https://nas.local:11434"] {
            let err = assert_endpoint_with(url, "t", resolves_to(&["93.184.216.34"])).unwrap_err();
            assert!(err.contains("SSRF"), "{url}: {err}");
        }
    }

    #[test]
    fn unresolvable_host_and_non_http_scheme_fail_closed() {
        assert!(assert_endpoint_with("https://nao-existe.invalid", "t", unresolvable).is_err());
        assert!(assert_endpoint_with("https://vazio.example", "t", resolves_to(&[])).is_err());
        assert!(assert_endpoint_with("file:///etc/passwd", "t", unresolvable).is_err());
        assert!(assert_endpoint_with("não é url", "t", unresolvable).is_err());
    }
}
