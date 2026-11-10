//! Feature `code_runner` — tool `code`, action `run_js`: executa JavaScript de verdade num
//! `boa_engine::Context` isolado (`sandbox.rs`), não só devolve texto que a LLM finge ter rodado.
//! Fecha a lacuna nomeada em `modules/plugins/mod.rs` ("nenhuma engine JS entra neste binário") e
//! no smoke real (`ctx/smoke-baseline.md` item 6): "escreve um código JS ... e roda" hoje não tem
//! tool nenhuma — a LLM só finge, ou chama `desktop` para abrir uma URL irrelevante.
//!
//! Diretório, não arquivo único, pela regra de 300 linhas do AGENTS.md: [`logger`] guarda o
//! `Logger` que captura `console.*` num buffer em vez de imprimir (o app não tem terminal para
//! ninguém ler), [`sandbox`] a construção do `Context` com os limites de runtime + o timeout de
//! parede, e [`tool`] a tool que o LLM chama, com os testes de contrato deste módulo.
//!
//! ## Por que `boa_engine` (puro Rust) e não `rquickjs`/`deno_core`
//! Pura Rust, sem `bindgen`/toolchain C — o mesmo motivo que barrou `espeakng` nesta árvore
//! (`Cargo.toml`, comentário do bloco de dependências de voz) não se aplica aqui. Licença
//! Unlicense OR MIT (verificada via `cargo info boa_engine`/`cargo info boa_runtime`, ambas
//! 0.22.0 em 2026-09-26), mesma família permissiva do resto do Cargo.toml.
//!
//! ## Por que roda COM aprovação (`register_destructive` em `tool.rs`)
//! O `Context` de `sandbox.rs` nunca ganha um objeto além de `console` (nenhum
//! `require`/`fetch`/`process`/acesso a arquivo — ver o teste `no_access_to_host_objects`), mas
//! isso não esgota o risco: `RuntimeLimits` do `boa_engine` 0.22 cobre iteração de loop, recursão e
//! pilha da VM, e SÓ isso — não existe teto de memória. Builtins nativos custeados pelo `.length`
//! de um array/string (ex. `Array.prototype.join`/`toLocaleString`) alocam fora do laço de opcode
//! que os limites acima contam, então um script pode estourar a memória do processo sem passar por
//! nenhum dos limites. Pior: o release é `panic = "unwind"` (`Cargo.toml` da raiz), então um PÂNICO
//! do lado do `boa_engine` morre só na task da sandbox (`spawn_blocking` devolve `JoinError`, que
//! `tool.rs` vira erro nomeado) — mas estouro de memória (`handle_alloc_error`, OOM do SO) e de pilha
//! nativa abortam o PROCESSO INTEIRO da Mika qualquer que seja a estratégia de panic.
//! `no_access_to_host_objects` prova ausência de bridge, não ausência de efeito no mundo real. Risco residual, não mitigado: a correção de
//! verdade é rodar `run_js` num processo filho com um teto de memória do próprio SO (Job Object no
//! Windows, `rlimit` no Unix), fora do escopo desta rodada. Até lá, o gate de aprovação
//! (`register_destructive`) é a única defesa entre um script hostil e o processo da Mika.
//!
//! ## Config-livre, primeira passada de registro
//! Sem `AppHandle`/`ConfigStore` nenhum — `register_tools()` é chamado direto de
//! `features::modules()` (`features/mod.rs`), mesmo padrão de `mood`/`desktop` (`process`/
//! `window`), sem precisar de uma segunda passada depois que o app monta.

mod logger;
mod sandbox;
mod tool;

pub(crate) use tool::register_tools;
