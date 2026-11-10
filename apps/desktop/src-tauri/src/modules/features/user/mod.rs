//! Feature `user` em Rust — port da tool `user_profile`
//! (`apps/desktop/src/features/user/tools/index.ts`, ações `get`/`query`/`learn`/`set`/`forget`/
//! `set_personality`).
//!
//! `config/schemas/user.rs` (outra frente) já porta o namespace `user` inteiro, então este módulo
//! não abre schema nenhum — só lê/escreve pelo `ConfigStore` que `register(store)` recebe.
//!
//! ## Tabela TS × Rust (remedida nesta rodada — duas premissas de cabeçalho anteriores caíram:
//! primeiro "`SocialMemoryService` não existe em Rust ainda", depois "não expõe as 3 escritas")
//!
//! | Ação | Depende de | Rust hoje |
//! |---|---|---|
//! | `get`/`query` | `ConfigStore` (`configProfile`) + `SocialMemoryService.get_graph()` | **portado** |
//! | `learn` | `ConfigStore` + `fillPreferredNameIfEmpty` (`SocialMemoryService.set_user_profile`) | **portado** |
//! | `set` | ESCRITA em `SocialMemoryService.set_user_profile` | **portado** — [`social_actions::handle_set`] |
//! | `forget` | ESCRITA em `SocialMemoryService.forget_user_profile` + `ConfigStore` (PII espelhado) | **portado** — [`social_actions::handle_forget`] |
//! | `set_personality` | ESCRITA em `SocialMemoryService.set_personality_profile` | **portado** — [`social_actions::handle_set_personality`] |
//!
//! ## As duas premissas que caíram, nesta ordem
//! 1. `modules::memory::social::SocialMemoryService` já existia e rodava em produção
//!    (`features::privacy::clear`) — o cabeçalho antigo dizia "não existe em Rust ainda".
//! 2. O serviço não expunha `setUserProfile`/`forgetUserProfile`/`setPersonalityProfile` — medido e
//!    confirmado numa rodada anterior (`set`/`forget`/`set_personality` degradavam nomeando essa
//!    lacuna). Nesta rodada o escopo se abriu para `modules/memory/social/user_profile.rs`, que
//!    porta as 3 escritas com a semântica de merge medida linha a linha em
//!    `apps/desktop/src/main/services/memory/social/user-profile-store.ts` (ver o cabeçalho
//!    daquele arquivo Rust para os detalhes: o que substitui, o que acumula, o que dedup).
//!
//! `set`/`forget`/`set_personality` só degradam agora quando `(self.social)()` devolve `None` — o
//! `MemoryStore` de processo não abriu (mesma causa nomeada por `no_social()`, nunca mais "a
//! escrita não existe").
//!
//! ## Duas divergências documentadas, não mascaradas
//!
//! 1. **Carimbo de data do `learn` não é localizado.** O TS usa
//!    `formatUserFacingDateTime(now, { dateStyle: 'short' })` — `Intl.DateTimeFormat` no locale/fuso
//!    do usuário (`app.locale`/`user.timezone`). Aqui a data é `AAAA-MM-DD` do dia no fuso do SO
//!    (`chrono::Local`, sem `chrono-tz`) — o dia é o mesmo, só o formato do carimbo muda.
//! 2. **`age` no perfil.** `config/schemas/user.rs` já documenta: o campo não tem "vazio"
//!    representável (`z.number().int().optional()` sem `.default()` no TS vira `undefined` quando
//!    limpo; o schema Rust usa o mínimo válido, `1`, como default). `action=get` aqui filtra pelo
//!    MESMO critério do TS (`val !== undefined && val !== ''`), então `age=1` nunca some da resposta
//!    mesmo quando o usuário nunca preencheu — o TS mostraria nada. Herdado do schema já portado,
//!    não introduzido por esta frente.
//!
//! `fillPreferredNameIfEmpty` (efeito colateral best-effort de `learn(category=name)`) tem porte
//! próprio em `social_actions.rs`, mas roda SÍNCRONO (sem fire-and-forget nem o contador
//! `profileGeneration` do TS) — ver o cabeçalho daquele arquivo para o porquê de a diferença de
//! timing não mudar o resultado observável.

mod feature;
mod plugin_test;
mod profile_config;
mod social_actions;
mod tool;

pub use tool::register;
/// Reexport estreito para `features::privacy::clear` reusar a MESMA lista de campos do perfil em
/// vez de duplicá-la — ver o comentário da constante em `tool.rs`.
pub(crate) use tool::USER_PROFILE_KEYS;
