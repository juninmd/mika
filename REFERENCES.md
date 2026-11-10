# Mika — Referências & Bibliografia Técnica

Documento canônico de referências, especificações e repositórios consultados pela arquitetura da Mika.

---

## 1. Documentação & Especificações Internas

| Documento | Localização | Descrição |
|---|---|---|
| Visão e Escopo | [`specs/00-vision-and-scope.md`](specs/00-vision-and-scope.md) | Propósito, persona, escopo e requisitos de produto do VTuber AI Companion |
| Arquitetura de Processos | [`specs/01-architecture.md`](specs/01-architecture.md) | Topologia Tauri 2, isolamento Renderer/Backend, barramento e IPC |
| Provedores de IA | [`specs/02-providers.md`](specs/02-providers.md) | Resolução de LLM local (Ollama) e remotos (OpenAI, Anthropic, Gemini, Groq) |
| Resiliência & Fallbacks | [`specs/03-resilience.md`](specs/03-resilience.md) | Estratégia de fallback, timeouts graduais, circuit-breaker e recuperação |
| Memória & Embeddings | [`specs/04-memory.md`](specs/04-memory.md) | SQLite, vetores semânticos com `multilingual-e5-small` e memória episódica |
| Catálogo de Pessoas | [`specs/05-person-catalog.md`](specs/05-person-catalog.md) | Reconhecimento de terceiros, vínculos sociais e contexto de conversa |
| Visão & Captura | [`specs/06-vision.md`](specs/06-vision.md) | Captura de tela multi-monitor, OCR, VLM e visual grounding |
| Persona & Afeto | [`specs/07-persona-and-relationship.md`](specs/07-persona-and-relationship.md) | Traços canônicos, afinidade dinâmica, humor e guardrail de repetição |
| Gamer Companion | [`specs/08-gamer-companion.md`](specs/08-gamer-companion.md) | Detecção de jogos, telemetria, comentários contextuais e cooldowns |
| Configuração & UI | [`specs/09-settings-and-ui.md`](specs/09-settings-and-ui.md) | Design system, tokens semânticos e Padrão Ouro UI/UX de Bento Grid |
| Observabilidade | [`specs/10-observability.md`](specs/10-observability.md) | Logs em anel, métricas locais, telemetria ZDR e redigimento de PII |
| Quality Gates | [`specs/11-quality-gates.md`](specs/11-quality-gates.md) | Catraca de invariantes, linters, testes de contrato e reach |
| Renderizador de Avatar | [`specs/12-avatar-render.md`](specs/12-avatar-render.md) | WebGL, renderização híbrida Live2D (Pixi) & VRM (Three.js), Lip-Sync |
| Voz & Turn-Taking | [`specs/13-voice-and-turn-taking.md`](specs/13-voice-and-turn-taking.md) | Áudio PCM16 mono 16kHz, VAD (Silero), STT (Whisper) e TTS (Kokoro/Supertonic) |
| Agent Harness | [`specs/14-agent-harness.md`](specs/14-agent-harness.md) | Loop de ferramentas, aprovação humana e sandbox de execução |

---

## 2. Tecnologias de Infraestrutura & Backend (Rust & Tauri)

- **Tauri 2**: Framework nativo multiplataforma com isolamento estrito de processos.
  - [Documentação Oficial Tauri v2](https://v2.tauri.app/)
  - [Repositório GitHub tauri-apps/tauri](https://github.com/tauri-apps/tauri)
- **ONNX Runtime (`ort`)**: Motor de inferência para VAD, STT, TTS e Embeddings via CPU e DirectML / CoreML.
  - [ONNX Runtime Releases](https://github.com/microsoft/onnxruntime)
  - [Crate Rust `ort`](https://github.com/pykeio/ort)
- **Boa Engine**: Motor ECMAScript embarcado seguro com limites de recursão, iteração e pilha.
  - [Boa JS Engine](https://github.com/boa-dev/boa)
- **Rig Core**: Abstração unificada para LLMs e execução de ferramentas em streaming.
  - [Rig Framework](https://github.com/0xPlaygrounds/rig)
- **Sysinfo**: Telemetria de hardware (RAM, Swap, CPU, Bateria).
  - [Crate Rust `sysinfo`](https://github.com/GuillaumeGomez/sysinfo)

---

## 3. Tecnologias de Frontend & UI (TypeScript & React)

- **React 19**: Biblioteca de componentes e ciclo de vida de renderização.
  - [React Docs](https://react.dev/)
- **Tailwind CSS v4**: Motor utilitário com sistema de tokens de cor semânticos e CSS variables.
  - [Tailwind CSS v4 Documentation](https://tailwindcss.com/)
- **Biome 2**: Linter e formatador de código de alta performance.
  - [Biomejs](https://biomejs.dev/)
- **Pixi.js v7**: Renderizador WebGL 2D para modelos Live2D Cubism 4.
  - [PixiJS](https://pixijs.com/)
- **Three.js & @pixiv/three-vrm**: Renderização 3D de avatares no padrão VRM 1.0.
  - [three-vrm Documentation](https://github.com/pixiv/three-vrm)

---

## 4. Modelos Neurais & Pesos Locais

- **Silero VAD**: Voice Activity Detection leve e preciso em ONNX.
  - [Silero VAD GitHub](https://github.com/snakers4/silero-vad)
- **Whisper**: Modelo de transcrição de voz (OpenAI Whisper via ONNX quantizado).
  - [OpenAI Whisper](https://github.com/openai/whisper)
- **Kokoro & Supertonic**: Modelos de síntese de fala expressivos e de baixa latência em ONNX.
  - [Kokoro-82M HuggingFace](https://huggingface.co/hexgrad/Kokoro-82M)
- **multilingual-e5-small**: Embeddings semânticos multilíngues de 384 dimensões em int8 ONNX.
  - [Xenova/multilingual-e5-small](https://huggingface.co/Xenova/multilingual-e5-small)
