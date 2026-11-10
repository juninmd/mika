//! Trace id por turno — porte do `setLogTrace` do renderer (`packages/sdk/src/core/logger-format.ts`
//! na era Node) para o shell Rust, lido por `log_buffer.rs::build_record` em cada `log::` emitido.
//!
//! **`tokio::task_local!`, não `thread_local!`.** Um turno roda como uma única `Future` que pode
//! ser retomada em outra worker thread do runtime do Tauri a cada `.await` (work-stealing) — um
//! `thread_local!` ficaria em branco assim que a continuação migrasse de thread, quebrando a
//! correlação exatamente no cenário que ela existe para cobrir (um turno com chamada de ferramenta,
//! LLM e IO no meio). `task_local!` amarra o valor à `Future` raiz de [`scoped`], não à thread.

tokio::task_local! {
    static TRACE_ID: String;
}

/// Novo id de turno — só precisa ser único o bastante para não colidir dentro de uma sessão local.
pub fn new_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("t-{nanos:x}")
}

/// Roda `fut` com `trace_id` visível a [`current`] em qualquer ponto do seu corpo, mesmo depois de
/// um `.await` que troca a thread que retoma a `Future`.
pub async fn scoped<F: std::future::Future>(trace_id: String, fut: F) -> F::Output {
    TRACE_ID.scope(trace_id, fut).await
}

/// Lido por `log_buffer.rs::build_record`. `None` fora de um turno (log de boot, config, etc.).
pub fn current() -> Option<String> {
    TRACE_ID.try_with(Clone::clone).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn absent_outside_any_scope() {
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn visible_inside_the_scope_and_gone_right_after() {
        let seen = scoped("t-abc".to_string(), async { current() }).await;
        assert_eq!(seen, Some("t-abc".to_string()));
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn survives_an_await_point_inside_the_scope() {
        let seen = scoped("t-xyz".to_string(), async {
            tokio::task::yield_now().await;
            current()
        })
        .await;
        assert_eq!(seen, Some("t-xyz".to_string()));
    }

    #[tokio::test]
    async fn nested_scopes_do_not_leak_into_each_other() {
        let outer = scoped("t-outer".to_string(), async {
            let inner = scoped("t-inner".to_string(), async { current() }).await;
            (current(), inner)
        })
        .await;
        assert_eq!(outer, (Some("t-outer".to_string()), Some("t-inner".to_string())));
    }
}
