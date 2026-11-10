//! Tabela ÚNICA de "endereço que o app nunca deve alcançar por pedido de busca/modelo/usuário" —
//! usada por `validate_navigable_url` (IP literal e nome), por `privacy::endpoint_guard` (DNS do
//! endpoint digitado) e pelo resolver do cliente HTTP guardado ([`PublicOnlyResolver`]).
//!
//! Por que um resolver, e não só validar a URL: validar o texto da URL não diz para onde o NOME
//! aponta. `evil.example` com A = `127.0.0.1` (ou um DNS que responde público na checagem e
//! privado na conexão — DNS rebinding) passaria por qualquer checagem prévia. O resolver é chamado
//! pelo próprio cliente NA HORA DE CONECTAR e descarta os endereços bloqueados, então não existe
//! janela entre "checar" e "usar". Limite conhecido: com proxy de sistema (`HTTP(S)_PROXY`) quem
//! resolve o destino é o proxy, não este processo.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || a == 0 // 0.0.0.0/8 ("esta rede")
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 CGNAT
        || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24 IETF protocol assignments
        || (a == 198 && (18..=19).contains(&b)) // 198.18.0.0/15 benchmarking
        || a >= 240 // 240.0.0.0/4 reservado
}

fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let tail = Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8);
    let nat64 = s[..6] == [0x64, 0xff9b, 0, 0, 0, 0]; // 64:ff9b::/96
    let sixtofour = s[0] == 0x2002; // 2002::/16 leva o IPv4 nos bits 16..48
    if nat64 {
        return Some(tail);
    }
    sixtofour.then(|| Ipv4Addr::new((s[1] >> 8) as u8, s[1] as u8, (s[2] >> 8) as u8, s[2] as u8))
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true; // ::1, ::, ff00::/8
    }
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_blocked_ipv4(mapped); // ::ffff:a.b.c.d
    }
    if s[..6] == [0; 6] {
        return true; // ::a.b.c.d (IPv4-compatible, obsoleto) — nunca é destino legítimo
    }
    if s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1 {
        return true; // 64:ff9b:1::/48 NAT64 de uso local
    }
    if let Some(v4) = embedded_ipv4(ip) {
        return is_blocked_ipv4(v4);
    }
    s[0] & 0xffc0 == 0xfe80 || s[0] & 0xfe00 == 0xfc00 // fe80::/10, fc00::/7
}

pub(crate) fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_blocked_ipv4(ip),
        IpAddr::V6(ip) => is_blocked_ipv6(ip),
    }
}

/// `localhost`, `*.localhost`, `*.local` (mDNS) e `*.internal` nunca são destino público,
/// independentemente do que o DNS responda. Ponto final (`localhost.`) não escapa.
pub(crate) fn is_blocked_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost" || [".localhost", ".local", ".internal"].iter().any(|suffix| host.ends_with(suffix))
}

/// Descarta os endereços bloqueados; se não sobrar nenhum, recusa (fail-closed) em vez de devolver
/// lista vazia que o cliente trataria como erro genérico de DNS.
pub(crate) fn public_addrs_only(addrs: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, String> {
    let kept: Vec<SocketAddr> = addrs.into_iter().filter(|addr| !is_blocked_ip(addr.ip())).collect();
    if kept.is_empty() {
        return Err("host resolve só para IP privado/local (SSRF)".to_string());
    }
    Ok(kept)
}

/// `reqwest::dns::Resolve` do cliente guardado: resolve pelo SO e só devolve endereço público.
pub(crate) struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            if is_blocked_hostname(&host) {
                return Err("nome local bloqueado (SSRF)".into());
            }
            let addrs = tokio::task::spawn_blocking(move || (host.as_str(), 0).to_socket_addrs().map(Iterator::collect::<Vec<_>>))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            Ok(Box::new(public_addrs_only(addrs)?.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::browser::validate_navigable_url;

    #[test]
    fn blocked_ip_table_covers_special_use_ranges() {
        for ip in [
            "0.0.0.0", "0.1.2.3", "100.64.0.1", "100.127.255.254", "192.0.0.8", "198.18.0.1", "198.19.255.255",
            "224.0.0.1", "239.255.255.250", "240.0.0.1", "255.255.255.254", "127.0.0.1", "10.1.1.1", "169.254.169.254",
            "::", "::1", "fe80::1", "fc00::1", "fd00::1", "ff02::1", "ff00::", "::127.0.0.1", "::10.0.0.1",
            "::ffff:10.0.0.1", "::ffff:169.254.169.254", "64:ff9b::7f00:1", "64:ff9b::a00:1", "64:ff9b::a9fe:a9fe",
            "64:ff9b:1::1", "2002:7f00:1::1", "2002:a9fe:a9fe::1",
        ] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "deveria bloquear {ip}");
        }
    }

    #[test]
    fn blocked_ip_table_does_not_over_block_public_neighbours() {
        for ip in [
            "8.8.8.8", "1.1.1.1", "100.63.255.255", "100.128.0.1", "192.0.2.1", "198.17.255.255", "198.20.0.1",
            "223.255.255.255", "93.184.216.34", "2606:4700:4700::1111", "64:ff9b::808:808", "2001:4860:4860::8888",
            "2002:808:808::1",
        ] {
            assert!(!is_blocked_ip(ip.parse().unwrap()), "não deveria bloquear {ip}");
        }
    }

    #[test]
    fn rejects_local_hostnames_and_new_ranges_before_any_dns() {
        for url in [
            "http://localhost/",
            "http://LOCALHOST:8080/x",
            "http://localhost./",
            "http://app.localhost/",
            "http://impressora.local/",
            "http://db.internal/",
            "http://a.b.internal./",
            "http://[64:ff9b::7f00:1]/",
            "http://100.64.0.1/",
            "http://198.18.0.1/",
            "http://224.0.0.1/",
            "http://2130706433/", // 127.0.0.1 em decimal
            "http://0x7f.1/",
        ] {
            assert!(validate_navigable_url(url).is_err(), "deveria bloquear {url}");
        }
        for url in ["http://notlocalhost.com/", "http://localhost.example.com/", "http://internal.example.com/", "http://local.example.org/"] {
            assert!(validate_navigable_url(url).is_ok(), "não deveria bloquear {url}");
        }
    }

    #[test]
    fn dns_filter_drops_blocked_addresses_and_fails_closed_when_none_left() {
        let addrs = |list: &[&str]| -> Vec<SocketAddr> { list.iter().map(|ip| SocketAddr::new(ip.parse().unwrap(), 0)).collect() };
        // Rebinding: o nome devolve um IP público E um privado — só o público sobrevive.
        let kept = public_addrs_only(addrs(&["10.0.0.5", "93.184.216.34", "127.0.0.1"])).unwrap();
        assert_eq!(kept, addrs(&["93.184.216.34"]));
        assert!(public_addrs_only(addrs(&["192.168.1.1", "::1", "64:ff9b::7f00:1"])).is_err());
        assert!(public_addrs_only(vec![]).is_err());
    }

    /// O resolver de verdade (o que o cliente guardado instala), sem rede externa: um "nome" que é
    /// um IP privado literal resolve para si mesmo pelo SO — e tem de ser recusado; o público passa.
    #[tokio::test]
    async fn resolver_refuses_private_answers_and_local_names_but_passes_public() {
        let resolve = |host: &str| PublicOnlyResolver.resolve(host.parse().unwrap());
        for host in ["127.0.0.1", "10.1.2.3", "169.254.169.254", "localhost", "app.localhost", "x.internal"] {
            assert!(resolve(host).await.is_err(), "deveria recusar {host}");
        }
        let kept: Vec<SocketAddr> = resolve("93.184.216.34").await.unwrap().collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].ip(), "93.184.216.34".parse::<IpAddr>().unwrap());
    }
}
