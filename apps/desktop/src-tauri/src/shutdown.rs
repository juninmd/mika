//! Sinal de desligamento de processo, compartilhado por todo loop de fundo em `modules/`.
//!
//! **O problema.** `specs/01-architecture.md` ("Ciclo de vida") exige que todo timeout, interval,
//! listener ou processo que um módulo inicia seja destruído quando o módulo desliga — mas o par
//! `activate()`/`deactivate()` do runtime de plugin TypeScript nunca foi portado para Rust. Cada
//! `tokio::spawn`/`tauri::async_runtime::spawn` de um loop de fundo (timers de tick, assinantes do
//! EventBus) rodava até o PROCESSO inteiro morrer, nunca até o módulo "desligar" — o que hoje só
//! acontece junto com o processo, mas sem um ponto único onde declarar essa intenção, o gate
//! `check:lifecycle-shutdown` (ver `modules/mod.rs`, seção de testes) não tem o que verificar.
//!
//! **A escolha.** Um `watch::channel<bool>` de processo, não um `CancellationToken` de
//! `tokio-util`: `tokio` já é dependência direta com a feature `sync` ligada (`Cargo.toml`), e um
//! `watch` dá exatamente o que falta — um único disparo (`trigger()`), lido por N assinantes
//! (`subscribe()`), sem precisar adicionar `tokio-util` só para isto. `bool` em vez de `()`: um
//! assinante que chega DEPOIS do disparo (`subscribe()` correndo com `trigger()` no shutdown) ainda
//! vê `true` em `*rx.borrow()` na primeira leitura, sem depender de nunca perder a mudança — a
//! garantia que `watch` dá e um `broadcast`/`mpsc` não dariam de graça.
//!
//! **O chamador.** `lib.rs`, `RunEvent::ExitRequested`/`RunEvent::Exit` — o mesmo ponto que já
//! dispara `modules::mcp::disconnect_all()` para matar o processo filho do MCP. `trigger()` é
//! idempotente (`watch::Sender::send` ignora erro de "sem receptor", e reenviar `true` não muda
//! nada), então não precisa de guarda contra chamada dupla.

use std::sync::OnceLock;
use tokio::sync::watch;

fn channel() -> &'static (watch::Sender<bool>, watch::Receiver<bool>) {
    static CHANNEL: OnceLock<(watch::Sender<bool>, watch::Receiver<bool>)> = OnceLock::new();
    CHANNEL.get_or_init(|| watch::channel(false))
}

/// Dispara o desligamento para todo assinante do processo. Chamado uma vez, do caminho de saída em
/// `lib.rs`. Reenviar depois de já ter disparado é inofensivo (idempotente).
pub fn trigger() {
    // `send` só falha sem receptor nenhum vivo — nada a fazer nesse caso, o processo já está
    // encerrando de qualquer forma.
    let _ = channel().0.send(true);
}

/// Um novo assinante do sinal de desligamento — chame uma vez por loop de fundo, antes do
/// `tokio::select!` que o mantém vivo.
pub fn subscribe() -> watch::Receiver<bool> {
    channel().1.clone()
}

/// Resolve quando [`trigger`] for chamado — inclusive se já tiver sido chamado antes deste
/// `subscribe()`. Uso: `tokio::select! { _ = shutdown::until_triggered(&mut rx) => break, ... }`.
pub async fn until_triggered(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            // Sender dropado (só acontece se este `OnceLock` for destruído, o que não acontece em
            // produção) — não há mais desligamento para esperar, mas também não há mais ninguém
            // para servir: melhor retornar do que travar o loop de fundo para sempre.
            return;
        }
    }
}

/// Teto de uma etapa do encerramento que espera por algo fora do processo (servidor MCP, rede).
pub const STEP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Espera `work` por no máximo `limit`; passou disso, loga e segue (`None`). O encerramento segura a
/// saída do app (`prevent_exit`) até suas etapas terminarem — uma etapa que não volta (servidor MCP
/// que não responde ao fechar) deixaria o app aberto para sempre depois do "Sair".
pub async fn within<F: std::future::Future>(step: &str, limit: std::time::Duration, work: F) -> Option<F::Output> {
    let outcome = tokio::time::timeout(limit, work).await.ok();
    if outcome.is_none() {
        log::warn!("encerramento: '{step}' passou de {}s, seguindo sem esperar", limit.as_secs());
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma etapa que nunca volta não pode segurar o encerramento além do teto.
    #[tokio::test]
    async fn a_shutdown_step_that_never_returns_is_abandoned_at_the_limit() {
        let bounded = within("etapa travada", std::time::Duration::from_millis(20), std::future::pending::<()>());
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(2), bounded).await.expect("o teto tem de soltar o encerramento");
        assert_eq!(outcome, None);
        assert_eq!(within("etapa rápida", std::time::Duration::from_secs(2), async { 7 }).await, Some(7));
    }

    // As duas provas abaixo usam um `watch::channel` LOCAL, nunca o `subscribe()`/`trigger()` de
    // processo: este módulo tem um único `OnceLock` por binário, `trigger()` não tem `reset()` (de
    // propósito — desligamento é um evento que só acontece uma vez na vida real), e `cargo test`
    // roda testes em paralelo no mesmo processo. Testar contra o canal de processo faria o
    // primeiro teste a chamar `trigger()` "vazar" para todos os outros testes deste binário que já
    // tiverem assinado. `until_triggered` não sabe a diferença entre o canal de processo e um local
    // — a prova vale igual.

    /// Prova mínima do mecanismo: um loop "de fundo" real, rodando em `tokio::spawn`, para quando o
    /// lado emissor manda `true` — sem isto, `until_triggered` seria só uma função que compila e
    /// nunca é exercitada de ponta a ponta dentro de um `select!`.
    #[tokio::test]
    async fn triggering_shutdown_stops_a_spawned_loop() {
        let (tx, mut rx) = watch::channel(false);
        let (tick_tx, mut tick_rx) = tokio::sync::mpsc::unbounded_channel::<u32>();

        let handle = tokio::spawn(async move {
            let mut ticks: u32 = 0;
            loop {
                tokio::select! {
                    _ = until_triggered(&mut rx) => break,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {
                        ticks += 1;
                        let _ = tick_tx.send(ticks);
                    }
                }
            }
            ticks
        });

        // Deixa o loop rodar pelo menos uma volta antes de desligar, para provar que ele estava
        // vivo (não que terminou antes do primeiro `select!`).
        tick_rx.recv().await.expect("o loop deveria ter marcado ao menos um tick antes do trigger");

        let _ = tx.send(true);

        let final_ticks = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("o loop deveria terminar logo após o trigger, não travar para sempre")
            .expect("a task não deveria ter entrado em pânico");

        assert!(final_ticks >= 1, "o loop precisa ter rodado antes de parar");
    }

    /// Um assinante que chega DEPOIS do disparo ainda vê o desligamento — não pode exigir estar
    /// inscrito ANTES do disparo para funcionar (ordem de boot entre módulos não é garantida).
    #[tokio::test]
    async fn a_late_subscriber_still_sees_a_shutdown_already_triggered() {
        let (tx, mut rx) = watch::channel(false);
        let _ = tx.send(true);

        tokio::time::timeout(std::time::Duration::from_millis(200), until_triggered(&mut rx))
            .await
            .expect("assinante tardio tem de ver o shutdown já disparado, sem travar");
    }
}
