# Licenças dos assets, modelos e runtimes

> **RASCUNHO — requer revisão jurídica antes da publicação.**

Este arquivo é o inventário do que o Mika carrega na tela e no ouvido, de onde veio, sob qual
licença, e — a pergunta que importa — **se vai dentro do instalador**. É a visão para quem mantém o
repositório. A visão para o usuário final (com os textos de licença integrais) é
`THIRD-PARTY-NOTICES.md`, gerada por `pnpm notices:gen` a partir de `scripts/lib/license-inventory.mts`
(constante `MANUAL_ASSETS`) mais os lockfiles. **Os dois têm de dizer a mesma coisa:** mudou um
asset aqui, mude em `MANUAL_ASSETS` e regenere — o gate `pnpm check:license-policy` reprova o aviso
desatualizado.

> **Por quê:** durante meses o `electron-builder` empacotava `files: ["out/**/*"]`, o que colocava
> 17 MB de modelos Live2D de terceiros e 11 MB de VRM dentro do instalador, sem opt-in, sem download
> sob demanda e sem nenhum arquivo dizendo o que aquilo era. Redistribuir material licenciado não é um
> detalhe de build. O `electron-builder` saiu na migração para Tauri; o risco não saiu com ele.

## Como o instalador é montado (fluxo Tauri)

1. `beforeBuildCommand` (`apps/desktop/src-tauri/tauri.conf.json`) roda `build:prompts` e
   `build:renderer`.
2. O Vite copia **todo** `apps/desktop/src/renderer/public/**` para `apps/desktop/out/renderer`.
3. O passo **`closeBundle`** do plugin `vite-plugin-tauri-bridge`, em
   `apps/desktop/vite.tauri.config.ts`, **apaga `out/renderer/live2d`** (`rmSync` recursivo) antes de o
   Tauri empacotar. É esse passo — e não um glob de exclusão — que impede os modelos Live2D de amostra
   de irem ao instalador: o `tauri.conf.json` não tem equivalente ao antigo `!out/renderer/live2d/**`.
4. O Tauri empacota `frontendDist` (`out/renderer`) e os `bundle.resources` do `tauri.conf.json`:
   os prompts, `renderer/vrm/` (para inspeção do modelo em Rust) e a pasta `legal/` (LICENSE,
   EULA, PRIVACY, THIRD-PARTY-NOTICES, este arquivo), que a tela Sobre/Licenças lê.

**Gate:** `pnpm check:packaged-assets` reprova (a) se `vite.tauri.config.ts` deixar de mencionar
`out/renderer/live2d`, (b) se existir em `public/live2d` uma pasta de modelo que não esteja neste
inventário e (c) se faltar o recurso `renderer/vrm/` no bundle Tauri. Limite honesto: o gate prova
que o passo existe, não que ele rodou. A prova de que rodou
é listar `apps/desktop/out/renderer` depois de `pnpm build` — o diretório `live2d` não pode existir.

Conferido em 2026-10-06: depois de `pnpm build`, `apps/desktop/out/renderer` contém `assets`, `index.html`, `live2dcubismcore.min.js`, `vad-processor.worklet.js` e `vrm` — **sem `live2d`**. (O `apps/desktop/src/renderer/public/vrm/mika/HARU.md`, nota interna de arte, vai junto por estar em `public/`: inofensivo, mas não é asset.)

## O que vai no instalador

| Asset                                                               | Origem                                                                                                                                                  | Licença                                                                                                                                                                                | Observação                                                                                                                                                                     |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/desktop/src/renderer/public/vrm/mika/mika.vrm`                | `Mika Aurora — Arcade`, derivada de `VRM1_Constraint_Twist_Sample` (c) 2022 pixiv Inc.                                                                  | VRM Public License 1.0; metadados embutidos: `commercialUsage=corporation`, redistribuição e modificação permitidas, crédito dispensável, uso antissocial e discurso de ódio proibidos | Termos retidos da base e lidos do bloco `VRMC_vrm.meta`; a base original está preservada em `scripts/generators/assets/mika-base.vrm`                                          |
| `apps/desktop/src/renderer/public/vrm/mika/haru.vrm`                | Derivado da base imutável `scripts/generators/assets/mika-base.vrm`; roupa e cabelo procedurais do projeto (`scripts/generators/generate-haru-vrm.mjs`) | Mesmos metadados da base                                                                                                                                                               | A regeneração não usa o `mika.vrm` oficial                                                                                                                                     |
| `apps/desktop/src/renderer/public/vrm/mika/preview-*.png`           | Miniaturas das skins, geradas pelo próprio app (`pnpm vrm:previews`)                                                                                    | Autoral                                                                                                                                                                                | Restam `original`, `haru` e `maid` |
| `apps/desktop/src/renderer/public/vrm/animations/*.vrma`            | Ver a próxima tabela e `CREDITS.md` na mesma pasta                                                                                                      | MIT e termos do VRoid Project                                                                                                                                                          | 14 clipes, todos referenciados em `vrm-clips.ts`                                                                                                                               |
| `apps/desktop/src/renderer/public/live2dcubismcore.min.js`          | Live2D Cubism Core (runtime JS)                                                                                                                         | Live2D Proprietary Software License Agreement                                                                                                                                          | É runtime, não material. Verificar se o faturamento exige o contrato de publicação do Cubism SDK                                                                               |
| Fontes `@fontsource` (Inter variável, IBM Plex Mono, Space Grotesk) | npm                                                                                                                                                     | SIL OFL 1.1                                                                                                                                                                            | Empacotadas localmente (sem Google Fonts); o texto da OFL vem em `THIRD-PARTY-NOTICES.md`                                                                                      |
| ONNX Runtime                                                        | Baixado pelo build do crate `ort`                                                                                                                       | MIT (Microsoft)                                                                                                                                                                        | No Windows o build inclui DirectML: verificar a redistribuição da DLL. CUDA/TensorRT não entram no instalador padrão                                                           |
| Ícone `apps/desktop/build/icon.png` e `icon.ico`                    | Gerado por `scripts/generators/generate-app-icon.mts`                                                                                                   | Autoral; direitos reservados a [RAZÃO SOCIAL]                                                                                                                                          | Confirmar a titularidade do avatar-base                                                                                                                                        |
| Dependências npm e Rust                                             | Lockfiles                                                                                                                                               | Listadas e verificadas por `pnpm check:license-policy` e `cargo deny`                                                                                                                  | Texto integral em `THIRD-PARTY-NOTICES.md`                                                                                                                                     |

### Clipes VRMA (`apps/desktop/src/renderer/public/vrm/animations`)

| Arquivos                                                                             | Origem                                                                       | Licença                                                                                                                             |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Angry, Blush, Clapping, Goodbye, LookAround, Relax, Sad, Sleepy, Surprised, Thinking | `tk256ailab/vrm-viewer`                                                      | MIT                                                                                                                                 |
| idle_loop                                                                            | `pixiv/ChatVRM`, via `semperai/amica`                                        | MIT                                                                                                                                 |
| modelPose, peaceSign, shoot                                                          | Conjunto VRMA oficial do VRoid Project, (c) pixiv Inc., via `semperai/amica` | Termos do VRoid Project; crédito obrigatório. **Termos não conferidos na fonte oficial: verificar uso comercial antes de publicar** |

Conferência de duplicatas (2026-10-06): os 10 clipes do `tk256ailab` têm o **mesmo tamanho**
(118 448 bytes) mas `md5sum` **diferentes** — não são cópias, só têm a mesma estrutura de trilhas.
Nenhum clipe é byte-idêntico a outro e todos os 14 são usados. Nada a apagar.

## O que é baixado sob demanda (não vai no instalador)

Baixados na primeira vez que o usuário liga o recurso, de revisões fixas (nunca `main`:
`pnpm check:pinned-downloads`), para a pasta de dados do app.

| Modelo                               | Origem                                                                 | Licença                                                                                                                                                    | Cache                        |
| ------------------------------------ | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| Whisper tiny/base/small (ONNX)       | `onnx-community/whisper-*` (revisão fixa em `whisper.rs`)              | Apache-2.0 (declarada em `openai/whisper-base`)                                                                                                            | `whisper-cache`              |
| Nemotron 3.5 ASR streaming 0.6B int4 | `onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4`             | **Declarações em conflito**: OpenMDW-1.1 (NVIDIA) × `license: mit` com `license_name: nvidia-open-model-license` (conversão). **Pendência jurídica**       | `nemotron-cache`             |
| Parakeet TDT 0.6B v3 int8            | `istupakov/parakeet-tdt-0.6b-v3-onnx` (revisão `8f23f0c`)              | CC-BY-4.0 (NVIDIA); conversão ONNX por istupakov. **Exige atribuição** — exibida no card do provedor                                                          | `parakeet-cache`             |
| Supertonic 3 (ONNX)                  | `supertone-oss-archive/supertonic-3` (arquivo oficial, revisão fixa)   | BigScience OpenRAIL-M (modelo), MIT (código de exemplo); (c) 2026 Supertone Inc. **Impõe restrições de uso repassáveis ao usuário** — incorporadas ao EULA | `supertonic/supertonic-3`    |
| Kokoro-82M v1.0 (ONNX)               | `onnx-community/Kokoro-82M-v1.0-ONNX`                                  | Apache-2.0                                                                                                                                                 | Opção hoje fora da interface |
| granite-embedding-97m-multilingual-r2 | `onnx-community/granite-embedding-97m-multilingual-r2-ONNX` (base `ibm-granite/…`) | Apache-2.0 (IBM)                                                                                                                               | `granite-cache`              |
| Silero VAD                           | `snakers4/silero-vad`, commit fixo                                     | MIT, Silero Team                                                                                                                                           | `vad-cache`                  |
| Vozes Piper                          | `rhasspy/piper-voices`, revisão `6b9cfaa` (vozes retreinadas)          | Repositório MIT; **a licença do dataset varia por voz**                                                                                                    | Cache de voz do Piper        |

**Piper — aviso comercial.** O ecossistema de vozes Piper mistura datasets com licenças diferentes,
e algumas vozes usam datasets **CC-BY-NC (uso não comercial)**, que **não podem ser oferecidas num
produto vendido**. As vozes oferecidas na interface (catálogo `voice/tts/providers/piper/catalog.rs`, cada uma com
SHA-256 do `.onnx` e do `.onnx.json` na revisão fixa `6b9cfaa` do repositório — nada baixa fora
dele), com a licença do dataset e da base de ajuste conferidas no `MODEL_CARD` em 2026-11-15:

| Voz                     | Idioma | Dataset                   | Licença do dataset           | Base do ajuste fino              |
| ----------------------- | ------ | ------------------------- | ---------------------------- | -------------------------------- |
| `pt_BR-faber-medium`    | pt     | OHF-Voice/voice-datasets  | CC0                          | `hifi_tts_92` (Hi-Fi TTS, CC BY 4.0 — exige atribuição) |
| `pt_BR-edresson-low`    | pt     | TTS-Portuguese-Corpus     | CC BY 4.0 (exige atribuição) | `en_US-ryan` (dataset não conferido) |
| `de_DE-thorsten-medium` | de     | Thorsten-Voice            | CC0                          | `rdh` (CC0)                      |
| `it_IT-paola-medium`    | it     | OHF-Voice/voice-datasets  | CC0                          | `hifi_tts_92` (Hi-Fi TTS, CC BY 4.0 — exige atribuição) |
| `ru_RU-denis-medium`    | ru     | OHF-Voice/voice-datasets  | CC0                          | `rdh` (CC0)                      |

**Linhagem resolvida (nov/2026).** `faber`, `thorsten`, `paola` e `denis` eram ajustes finos do
checkpoint `en_US-lessac` (dataset Lessac/Blizzard 2013, só pesquisa). Foram retreinadas a partir das
bases `hifi_tts_92` e `rdh`, de licença comercial, e o catálogo passou a baixar só essas versões (que
exigem **Piper 1.5+**; um Piper que se declara mais antigo é recusado com o motivo). As cópias antigas
ficam fora do cache novo e nunca são reaproveitadas. `edresson` não foi retreinada e continua partindo
de `en_US-ryan`, cujo dataset não foi conferido. `en_US-lessac-high` e `en_US-amy-medium` seguem
**fora da interface e do catálogo**.

**Idiomas sem voz Piper oferecida, de propósito:** `en` (o Supertonic já o fala; desde o Supertonic 3 ele também fala de, it, ja, ko e ru), `ja` e `ko` (o Piper não tem voz nesses idiomas) e
`zh` (`zh_CN-huayan`: licença do dataset "Unknown"). Também não entram `ru_RU-ruslan` e `de_DE-pavoque`
(CC BY-NC-SA) nem `ru_RU-irina` ("Unknown"). Nesses idiomas, com Supertonic ou Piper ativo, a aba de
voz mostra um aviso e manda escolher um provedor de nuvem (Azure Speech, Google Cloud, ElevenLabs).

O binário do Piper não é distribuído: o usuário o instala. Antes de ampliar a lista de vozes, leia o
`MODEL_CARD` de cada uma **e** de onde ela foi ajustada.

## Modelo próprio (Live2D ou VRM)

Nenhum modelo Live2D vem no repositório nem no instalador. O app abre no VRM oficial; para usar
outro avatar, **Configurações → Skin → Importar modelo** abre o seletor do sistema, aceita um `.vrm`
ou o `.model3.json` de um Live2D Cubism e copia o modelo (a pasta inteira, no Live2D) para
`<pasta de dados>/live2d/<nome>/` (no Windows, `%APPDATA%/com.mika.desktop/live2d/<nome>/`), a pasta
que o protocolo `mika-live2d` serve. A licença do modelo importado é responsabilidade de quem importa.

Um modelo ausente ou quebrado falha com erro nomeado na tela, nunca com um avatar em branco.

## Só no build Steam

A feature `steam` do crate liga o Steamworks e leva `steam_api64.dll` / `libsteam_api.so` ao depot.
São redistribuíveis apenas nos termos do **Valve Steamworks SDK Access Agreement**; o binding
`steamworks` é MIT/Apache-2.0. Sem a feature, nenhum código nem DLL da Steamworks entra no binário.

## Regra para asset novo

**MUST**: asset de terceiro entra neste inventário e em `MANUAL_ASSETS`
(`scripts/lib/license-inventory.mts`) no mesmo commit em que entra na árvore, com origem, licença e a
decisão de distribuir; depois `pnpm notices:gen`. Gates: `pnpm check:packaged-assets` (Live2D fora do
bundle, modelo no inventário) e `pnpm check:license-policy` (aviso de terceiros atual, nenhuma licença
proibida nas dependências).
