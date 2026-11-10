# Plano: atualização de dependências, parametrização e GPU

Levantado em 2026-09-25 por leitura (`pnpm outdated -r`, `cargo update --dry-run`, API do crates.io,
fonte do `ort-2.0.0-rc.13` no registry). **Nada foi instalado nem executado.** Host de referência:
RTX 4060 Ti 16 GB, driver 610.88, Node 24.19, pnpm 10.34.5, cargo 1.97.1.

Ordem de execução: Fase 1 → 2 → 3 → 4, uma fase por commit, com a validação da §4 do AGENTS.md
entre elas. Fase 5 é decisão, não tarefa.

## Resultado (executado em 2026-09-25)

Fases 1, 2 e 4 aplicadas como planejado. Na Fase 3, a medição mudou o plano da §3.4:

| Modelo                 | Decisão            | Medido (RTX 4060 Ti, com o LLM local carregado)                    |
| ---------------------- | ------------------ | ------------------------------------------------------------------ |
| Whisper encoder (int8) | GPU                | p50 453 → 89 ms; 1ª execução ~1,5 s (aquecimento do DirectML)      |
| Whisper decoder (int8) | CPU                | sem ganho na GPU, uma falha e um outlier de 153 s em 3 rodadas A/B |
| Supertonic (3 grafos)  | GPU                | ~556 → ~217 ms por frase em regime; carga +0,7 s uma vez           |
| Silero VAD             | CPU, 1 thread      | janela de 32 ms; GPU só somaria cópia                              |
| Nemotron (int4)        | CPU (outra frente) | DirectML recusa MatMulNBits no commit (E_INVALIDARG)               |
- A queda para CPU cobre também o `commit`, não só o registro do EP: o Nemotron int4 registra o
  DirectML e só estoura ao montar a sessão. Teste: `inference::session` (`--ignored`, modelo real).
- Paridade: Silero GPU×CPU (`gpu_session_matches_cpu_on_the_real_model`), Whisper com o texto de
  referência 3/3, Supertonic com áudio audível (`real_model_synthesizes_audible_speech`).
- Bundle do renderer com Oxc: 9.944.229 → 9.821.361 bytes.
- Não feito: toggle `inference.device` na UI (§3.5). A queda automática para CPU cobre falha de
  driver; contenção de VRAM com o LLM continua sem controle do usuário.

---

## Fase 1 — patch/minor sem risco (JS e Rust)

JS (`pnpm up -r` dentro do range, regenera o lockfile):

| Pacote                      | Atual → alvo                                                                         |
| --------------------------- | ------------------------------------------------------------------------------------ |
| vitest, @vitest/coverage-v8 | 5.0.1 → 5.0.2 (subir juntos: mesma versão exata)                                     |
| vite                        | 8.3.0 → 8.3.1                                                                        |
| tsx                         | 4.23.13 → 4.23.15                                                                    |
| turbo                       | 2.10.13 → 2.11.4                                                                     |
| @types/node                 | 26.2.0 → 26.6.3                                                                      |
| framer-motion               | 13.4.0 → 13.4.4                                                                      |
| lucide-react                | 1.46.0 → 1.48.0                                                                      |
| dompurify                   | 3.4.15 → 3.4.16 (sanitizador: subir sempre)                                          |
| three                       | 0.186.0 → 0.186.1 (pin exato; conferir `@pixiv/three-vrm` 3.5.5 continua compatível) |

Rust (`cargo update`, só lockfile): 25 crates, entre eles `rmcp` 3.4.0→3.4.1, `thiserror`,
`hyper-util`, `zerocopy`, `wasm-bindgen`; `multiversion` sai do grafo.

## Fase 2 — majors Rust que DEDUPLICAM o grafo

Hoje o grafo compila duas versões de cada; subir a nossa remove a cópia velha.

| Crate              | Atual → alvo | Por quê                                          | Risco                                                                                                                      |
| ------------------ | ------------ | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `reqwest`          | 0.12 → 0.13  | `rig-core` e `tauri-plugin-updater` já usam 0.13 | feature `rustls-tls` virou `rustls` na 0.13 (conferido no `Cargo.toml` do 0.13.5); `json`/`stream`/`cookies` mantêm o nome |
| `sha2`             | 0.10 → 0.11  | `scrypt` 0.12 e `zip` já usam 0.11               | API `digest` 0.11 (`finalize` → `Array`); só o verificador de download usa                                                 |
| `starship-battery` | 0.10 → 0.11  | versão mantida                                   | ler changelog                                                                                                              |

**Não subir isolado:** `webview2-com` (0.38 → 0.39) e `windows-core` (0.61) seguem a versão que o
`wry` travou — subir sozinho duplica `windows-strings` (ver comentário no `Cargo.toml`). Só sobem
quando o Tauri subir o `wry`.

Pré-releases fixados (`ort` rc.13, `tokenizers` 1.0.0-rc.2, `zip` 9.0.0-pre3): já estão no topo;
nada a fazer. `tauri` 3.0.0-alpha: **não** — alpha.

Higiene do manifesto: `rust-version` estava em `1.77` (o código usa `LazyLock`, 1.80, e o grafo de
dependências exige até 1.95 — `sysinfo`); agora é `1.95`, o maior `rust-version` do `cargo metadata`.
Ao subir dependências, reconfira. Edition 2024 é migração separada
(`cargo fix --edition`), fora deste plano.

## Fase 3 — GPU e parametrização da inferência (o ganho real)

### 3.1 Estado atual

As cinco sessões ONNX — Whisper encoder/decoder (`inference/whisper.rs:439-440`), Nemotron
(`inference/nemotron/mod.rs:46`), Silero VAD (`inference/vad.rs:117`), Supertonic
(`voice/tts/providers/supertonic/model.rs:52`) — são `Session::builder()?.commit_from_file()` puro:
**CPU only**, otimização de grafo padrão, threads padrão. A GPU fica ociosa para STT/TTS.

### 3.2 Execution providers disponíveis no `ort` rc.13 (binário pré-compilado do pyke)

| Alvo                     | Feature              | Observação                                                                                                          |
| ------------------------ | -------------------- | ------------------------------------------------------------------------------------------------------------------- |
| x86_64-pc-windows-msvc   | `directml`           | o binário padrão do Windows já é o build DirectML — ativar a feature não troca o download; roda em NVIDIA/AMD/Intel |
| x86_64-pc-windows-msvc   | `cuda` (+`tensorrt`) | só CUDA 13; exige CUDA/cuDNN no PC do usuário — inviável como padrão de instalador                                  |
| aarch64-apple-darwin     | `coreml`             | Apple Silicon                                                                                                       |
| x86_64-unknown-linux-gnu | `webgpu` ou `cuda`   | `webgpu` é o único sem dependência de toolkit                                                                       |

Proposta de `Cargo.toml` (feature por alvo, sem CUDA no padrão):

```toml
ort = "2.0.0-rc.13"

[target.'cfg(target_os = "windows")'.dependencies]
ort = { version = "2.0.0-rc.13", features = ["directml"] }

[target.'cfg(target_os = "macos")'.dependencies]
ort = { version = "2.0.0-rc.13", features = ["coreml"] }
```

### 3.3 Builder único (um só lugar, não cinco cópias)

Um `inference/session.rs` com `fn build_session(path, profile) -> ort::Result<Session>` que os
cinco chamadores usam:

```rust
Session::builder()?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_intra_threads(profile.intra_threads)?   // ver 3.4
    .with_intra_op_spinning(false)?               // não queimar CPU entre chunks de áudio
    .with_execution_providers(eps_for(profile.device))?  // EP com fallback para CPU
    .commit_from_file(path)
```

Regras por EP (documentadas pelo ONNX Runtime):
- **DirectML:** `with_memory_pattern(false)` e execução sequencial (`with_parallel_execution(false)`),
  senão o EP falha ou cai para CPU.
- **Fallback:** `with_execution_providers` registra em ordem e cai para CPU se o EP não sobe; logar
  qual EP ficou ativo (senão "GPU ligada" vira afirmação sem prova — mesma classe de
  `project_resposta_que_significa_nao_fiz`).

### 3.4 O que vai para a GPU e o que fica na CPU

| Modelo             | Destino           | Motivo                                                 |
| ------------------ | ----------------- | ------------------------------------------------------ |
| Whisper encoder    | **GPU**           | maior custo por turno; ganho grande                    |
| Nemotron streaming | **GPU** (medir)   | chunks pequenos: latência de cópia pode anular o ganho |
| Supertonic TTS     | **GPU**           | tempo-até-primeira-fala                                |
| Whisper decoder    | medir             | autoregressivo, tensores pequenos                      |
| Silero VAD         | **CPU, 1 thread** | 30 ms por janela; GPU só adiciona latência de cópia    |

### 3.5 Contenção de VRAM com o LLM local

O LLM local (Ollama / llama-server com Gemma) já disputa os 16 GB (ver memória
`project_teste_real_gemma_llama_server`: VRAM no limite). GPU para STT/TTS **não** pode ser
ligada às cegas. Proposta:

- ConfigKey `inference.device`: `auto | gpu | cpu` (padrão `auto`), com UI na aba de voz
  (bilateralidade UI↔backend, §3.2 do AGENTS.md) e `StatusPill` mostrando o EP efetivo.
- `auto` = GPU só se houver VRAM livre acima de um piso; senão CPU.
- `inference.intra_threads`: padrão `min(4, núcleos físicos / 2)` para não brigar com o LLM em CPU.

### 3.6 Prova (QA) antes de declarar ganho

Medir antes/depois, mesmo áudio fixture, 20 execuções, p50/p95: latência de STT por turno,
tempo-até-primeiro-áudio do TTS, VRAM de pico com o LLM carregado. Teste `cargo test` que
confirma fallback para CPU quando o EP não sobe. Budget sugerido: STT p95 cair ≥ 40% ou a mudança
não compensa a complexidade.

## Fase 4 — parametrização de build e renderer

| Onde                        | Hoje                                  | Proposta                                                                                                                                                  | Prova                                  |
| --------------------------- | ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| `[profile.release]`         | `opt-level = "s"`                     | `opt-level = 3` — mel/FFT do Nemotron, `rubato` e JPEG da visão rodam em Rust puro e "s" corta vetorização                                                | tamanho do exe e latência antes/depois |
| `[profile.dev.package."*"]` | ausente                               | `opt-level = 2` só nas dependências: `image`, `rubato`, `png` em opt 0 são 10×+ mais lentos no `tauri dev`, e o próprio crate continua rápido de compilar | tempo do encode de screenshot em dev   |
| `vrm-adapter.ts:76`         | `WebGLRenderer` sem `powerPreference` | `powerPreference: 'high-performance'` (o Live2D já usa) — em notebook híbrido o VRM cai na iGPU                                                           | FPS em notebook híbrido                |
| `vite.tauri.config.ts`      | `minify: 'esbuild'`, `rollupOptions`  | remover `minify`/`cssMinify` (Vite 8 usa Oxc por padrão; `'esbuild'` é depreciado) e renomear `rollupOptions` → `rolldownOptions`                         | diff do tamanho do bundle; E2E smoke   |
| `manualChunks`              | ativo                                 | manter até o Vite remover; memória `project_bundle_weight_v3` registra que ele des-lazia o mermaid — revisar junto                                        | tamanho do chunk inicial               |

## Fase 5 — decisões que não são upgrade

- **`pixi.js` 6 → 8: bloqueado.** `pixi-live2d-display` 0.4.0 (última) exige pixi `^6`. Pixi 8 só
  com troca de engine Live2D (`untitled-pixi-live2d-engine` 1.4.0, peer `pixi.js ^8.13`) — é
  reescrita do adaptador Live2D, não bump. Ganho: renderer WebGPU do pixi 8. Decidir se vale.
- **GPUI (Zed): não recomendado.** GPUI é um framework de UI nativo em Rust que substituiria a
  WebView inteira. Mika renderiza avatar com three.js/`@pixiv/three-vrm` e Live2D (Cubism Web),
  que não existem em GPUI; seria reescrever renderer, avatar e as 20+ telas de settings. O gargalo
  medível hoje não é a UI, é a inferência em CPU (Fase 3). Se o objetivo era "usar a GPU", a Fase 3
  entrega isso; a UI já é acelerada por GPU no WebView2/WebKit.
