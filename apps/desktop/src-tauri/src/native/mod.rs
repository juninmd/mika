//! Registro dos módulos que já foram portados do sidecar Node para Rust.
//!
//! A migração não troca o backend de uma vez — ela mata o sidecar por ATRITO. `bridge.rs` pergunta
//! a este registro se o canal tem dono nativo; se tiver, resolve aqui, e só o que ninguém reclamou
//! desce para o sidecar. Cada módulo portado tira seus canais daquela seta, e quando ela fica vazia
//! o sidecar é apagado.
//!
//! Por que assim, e não uma reescrita de uma vez: o app fica funcionando em todo commit, cada
//! módulo é verificável sozinho, e o progresso vira um número que só pode cair —
//! `scripts/checks/sidecar-attrition.mts` é a catraca.
//!
//! Os canais de shell (janela, tray, plugin-view) NÃO passam por aqui: eles nunca estiveram no
//! sidecar, são do Rust desde o início, e continuam em `bridge.rs`.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use tauri::AppHandle;

use crate::protocol::InvokeContext;

/// Um módulo de backend que já vive em Rust.
///
/// `channels()` é a fonte da verdade do que este módulo reivindica. Um canal declarado aqui e não
/// tratado em `invoke` vira erro explícito em vez de cair no sidecar — o silêncio é justamente o
/// modo de falha que custou caro nesta migração (ação que descia para o sidecar e nunca acontecia).
#[async_trait]
pub trait NativeModule: Send + Sync {
    fn name(&self) -> &'static str;

    fn channels(&self) -> &'static [&'static str];

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String>;
}

/// Tabela canal → módulo, montada uma vez no boot.
pub struct NativeRegistry {
    modules: Vec<Box<dyn NativeModule>>,
    by_channel: HashMap<&'static str, usize>,
}

impl NativeRegistry {
    /// Monta o registro. Canal declarado por dois módulos é ERRO DE PROGRAMAÇÃO e derruba o boot:
    /// resolver por "o último ganha" faria uma rota inteira mudar de dono sem ninguém perceber.
    pub fn new(modules: Vec<Box<dyn NativeModule>>) -> Self {
        let mut by_channel = HashMap::new();

        for (index, module) in modules.iter().enumerate() {
            for channel in module.channels() {
                if let Some(previous) = by_channel.insert(*channel, index) {
                    panic!(
                        "canal '{channel}' reivindicado por '{}' e por '{}'",
                        modules[previous].name(),
                        module.name()
                    );
                }
            }
        }

        Self { modules, by_channel }
    }

    pub fn owns(&self, channel: &str) -> bool {
        self.by_channel.contains_key(channel)
    }

    pub async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        let index = *self
            .by_channel
            .get(channel)
            .ok_or_else(|| format!("canal '{channel}' não tem dono nativo"))?;

        self.modules[index].invoke(app, ctx, channel, payload).await
    }
}

/// Todos os módulos nativos de hoje. Cada fase da migração acrescenta uma linha aqui.
/// Ver `docs/tauri-rust-migration.md` para a ordem e o porquê dela.
pub fn registry() -> NativeRegistry {
    NativeRegistry::new(crate::modules::all())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(&'static str, &'static [&'static str]);

    #[async_trait]
    impl NativeModule for Fake {
        fn name(&self) -> &'static str {
            self.0
        }
        fn channels(&self) -> &'static [&'static str] {
            self.1
        }
        async fn invoke(
            &self,
            _app: &AppHandle,
            _ctx: &InvokeContext,
            channel: &str,
            _payload: Value,
        ) -> Result<Value, String> {
            Ok(Value::String(format!("{}:{channel}", self.0)))
        }
    }

    #[test]
    fn owns_only_declared_channels() {
        let registry = NativeRegistry::new(vec![Box::new(Fake("a", &["x:one", "x:two"]))]);

        assert!(registry.owns("x:one"));
        assert!(registry.owns("x:two"));
        assert!(!registry.owns("x:three"));
        assert_eq!(registry.by_channel.len(), 2);
    }

    /// Dois donos para o mesmo canal não podem "resolver sozinho": a rota mudaria de dono em
    /// silêncio e o módulo perdedor ficaria morto sem nenhum erro.
    #[test]
    #[should_panic(expected = "reivindicado por")]
    fn duplicate_channel_is_a_boot_failure() {
        NativeRegistry::new(vec![
            Box::new(Fake("a", &["x:dup"])),
            Box::new(Fake("b", &["x:dup"])),
        ]);
    }

    /// O registro REAL monta, e monta inteiro.
    ///
    /// Duas coisas de uma vez: `NativeRegistry::new` roda de verdade sobre os módulos de produção
    /// (é lá que dois donos para o mesmo canal derrubam o boot — aqui isso vira teste vermelho em
    /// vez de app que não abre no usuário), e a soma bate com o total declarado. Se batesse menos,
    /// algum canal teria sido engolido por colisão silenciosa.
    ///
    /// A versão anterior deste teste cobrava `channel_count() == 0`, o que só valia enquanto
    /// nenhuma fase tivesse migrado nada; ele envelheceu no primeiro módulo portado.
    #[test]
    fn production_registry_claims_every_declared_channel() {
        let declared: usize = crate::modules::all()
            .iter()
            .map(|module| module.channels().len())
            .sum();

        assert_eq!(registry().by_channel.len(), declared);
    }
}
