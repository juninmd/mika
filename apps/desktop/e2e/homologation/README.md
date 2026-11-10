# Homologação funcional — features do roster

Executa as **tools** das features no app real, contra o **Ollama local**, e escreve um relatório com
o veredito de cada verificação. Não substitui o E2E de `e2e/specs`: aquele é determinístico e usa um
dublê de LLM; este exercita o stack de verdade, e homologar contra um dublê não homologa nada.

```bash
pnpm --filter @mika/desktop homologate:features
```

Um spec por vez:

```bash
cd apps/desktop
npx playwright test -c e2e/homologation/playwright.config.ts features.spec.ts
```

## O que cada spec faz

| Spec | O que prova |
| --- | --- |
| `catalog.spec.ts` | O que o app REGISTROU depois do boot (o roster diz o que deveria existir; isto diz o que existe). Grava o catálogo com os `inputSchema` reais em `report/` (gerado pelo spec, fora do git). |
| `features.spec.ts` | A matriz das features do roster, com round-trip onde dá (grava → lê → apaga). Gera `report/features.md`. |
| `features-core.spec.ts` | As tools do núcleo (brain, voice, vision) e as do companion que a matriz do roster não toca — visão capturando e descrevendo a tela de verdade. Gera `report/features-core.md`. |
| `tool-routing.spec.ts` | O MODELO escolhendo a ferramenta sozinho, que é justamente o que o dublê não consegue provar. Gera `report/tool-routing.md`. |
| `voice-loop.spec.ts` | O laço de áudio completo: TTS → VAD → Whisper → LLM → TTS. |
| `prompt-benchmark.spec.ts` | A/B do `prompts/brain/system.md`: a seção de roteamento em prosa paga os tokens que custa? A variante B é o próprio arquivo sem a seção, aplicada como override do perfil. |
| `settings-paths.spec.ts` | Cada caminho de configuração declarado chega ao store. |
| `probe-defaults.spec.ts` | Os defaults de produção do registro de config (privacidade: web/computer-use/telemetria desligados; o local, ligado). Grava `screenshots/probe-defaults.json`. |

## Variáveis

| Variável | Para quê |
| --- | --- |
| `MIKA_HOMOLOG_PROFILE` | Diretório do perfil. Padrão `%APPDATA%\Mika\desktop-homolog` — **não** é o perfil de uso, porque a matriz executa `clear_history`. |
| `MIKA_HOMOLOG_FRESH=1` | Sobe sem `config.json` nenhum: é a única forma de observar o primeiro boot de uma instalação limpa. |
| `MIKA_HOMOLOG_MODEL` | Modelo de chat. Padrão `qwen3.5:4b` (3,4 GB, com `tools` e `vision` na mesma pesagem). |
| `MIKA_HOMOLOG_VISION_MODEL` | Modelo de visão. Padrão: o mesmo do chat. |
| `MIKA_HOMOLOG_NO_VISION=1` | Sobe com visão desligada — para separar defeito de visão de defeito de captura. |
| `MIKA_BENCH_PROVIDER` / `MIKA_BENCH_MODEL` | Só o `prompt-benchmark`: por padrão usa o Ollama local; aponte para um provedor pago aqui, sabendo que é pago. |
| `MIKA_HOMOLOG_OLLAMA` | Base URL do Ollama. Padrão `http://127.0.0.1:11434`. |

## Vereditos

- **ok** — a feature fez o que promete.
- **sem-credencial** — só funciona com credencial externa que esta máquina não tem. O que se prova é
  que o erro é claro em vez de estouro (ou que uma feature desligada fica fora do catálogo do LLM).
- **bloqueado-externo** — o terceiro é que não respondeu. Não é defeito do produto e não passa por bom.
- **falha** — defeito.

Marcar tudo verde é a forma mais rápida de uma homologação virar enfeite: os três vereditos que não
são `ok` existem para que o que não foi provado continue visível.

## Duolingo: o login é manual, uma vez

A Mika dirige um Chrome com perfil próprio em `<perfil>/plugins/mika.duolingo`. Estar logado no seu
Chrome do dia a dia não vale. Para provar o caminho feliz (exercícios resolvidos), entre uma vez
naquele Chrome — ele abre visível durante a lição — e rode de novo. Sem login, a lição para no
primeiro passo com uma frase dizendo isso, que é o comportamento correto.
