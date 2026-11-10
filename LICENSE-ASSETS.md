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
| `apps/desktop/src/renderer/public/vrm/mika/preview-*.png`           | Miniaturas das skins, geradas pelo próprio app (`pnpm vrm:previews`)                                                                                    | Autoral                                                                                                                                                                                | Os nomes `hiyori`, `mao`, `mark`, `rice` são presets de cor do VRM, **não** têm relação com os modelos Live2D de mesmo nome; considerar renomear para evitar confusão de marca |
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
| Supertonic-TTS-2 (ONNX)              | `onnx-community/Supertonic-TTS-2-ONNX` (base `Supertone/supertonic-2`) | BigScience OpenRAIL-M (modelo), MIT (código de exemplo); (c) 2026 Supertone Inc. **Impõe restrições de uso repassáveis ao usuário** — incorporadas ao EULA | `supertonic`                 |
| Kokoro-82M v1.0 (ONNX)               | `onnx-community/Kokoro-82M-v1.0-ONNX`                                  | Apache-2.0                                                                                                                                                 | Opção hoje fora da interface |
| multilingual-e5-small                | `Xenova/multilingual-e5-small`                                         | MIT (declarada em `intfloat/multilingual-e5-small`); cartão da conversão sem licença própria                                                               | `e5-cache`                   |
| Silero VAD                           | `snakers4/silero-vad`, commit fixo                                     | MIT, Silero Team                                                                                                                                           | `vad-cache`                  |
| Vozes Piper                          | `rhasspy/piper-voices` v1.0.0                                          | Repositório MIT; **a licença do dataset varia por voz**                                                                                                    | Cache de voz do Piper        |

**Piper — aviso comercial.** O ecossistema de vozes Piper mistura datasets com licenças diferentes,
e algumas vozes usam datasets **CC-BY-NC (uso não comercial)**, que **não podem ser oferecidas num
produto vendido**. As vozes oferecidas na interface (catálogo `voice/tts/providers/piper/catalog.rs`, cada uma com
SHA-256 do `.onnx` e do `.onnx.json` na revisão fixa `375a0fe` do repositório — nada baixa fora
dele), com a licença do dataset conferida na origem em 2026-10-06:

| Voz                     | Idioma | Dataset                               | Licença                      |
| ----------------------- | ------ | ------------------------------------- | ---------------------------- |
| `pt_BR-faber-medium`    | pt     | NabuCasa/voice-datasets               | CC0                          |
| `pt_BR-edresson-low`    | pt     | TTS-Portuguese-Corpus                 | CC BY 4.0 (exige atribuição) |
| `de_DE-thorsten-medium` | de     | Thorsten-Voice                        | CC0                          |
| `it_IT-paola-medium`    | it     | `paolapersico1/Voice-Dataset-Italian` | CC0-1.0 (cartão do dataset)  |
| `ru_RU-denis-medium`    | ru     | NabuCasa/voice-datasets               | CC0                          |

**Pendência jurídica de linhagem.** `faber`, `thorsten`, `paola` e `denis` são ajustes finos do
checkpoint `en_US-lessac`, treinado no dataset Lessac/Blizzard 2013, cuja licença é só para pesquisa e
exclui "qualquer finalidade comercial, inclusive o desenvolvimento de produtos de síntese de voz".
Os datasets de cada voz são livres, mas os pesos partem de um checkpoint treinado em dado restrito:
isso precisa de parecer antes de vender (`edresson` parte do checkpoint `en_US-ryan`, cujo dataset não foi
conferido). `en_US-lessac-high` (treinada direto nesse dataset) e `en_US-amy-medium` (licença do dataset não verificada) foram
**retiradas da interface e do catálogo** (quem as tinha salvas volta ao default `pt_BR-faber-medium`). Voz treinada do zero com licença verificada seria a saída
(`de_DE-eva_k`/`it_IT-riccardo` são x_low e seu dataset M-AILABS não pôde ser conferido).

**Idiomas sem voz Piper oferecida, de propósito:** `en` (o Supertonic já o fala), `ja` e `ko` (o Piper não tem voz nesses idiomas) e
`zh` (`zh_CN-huayan`: licença do dataset "Unknown"). Também não entram `ru_RU-ruslan` e `de_DE-pavoque`
(CC BY-NC-SA) nem `ru_RU-irina` ("Unknown"). Nesses idiomas, com Supertonic ou Piper ativo, a aba de
voz mostra um aviso e manda escolher um provedor de nuvem (Azure Speech, Google Cloud, ElevenLabs).

O binário do Piper não é distribuído: o usuário o instala. Antes de ampliar a lista de vozes, leia o
`MODEL_CARD` de cada uma **e** de onde ela foi ajustada.

## O que NÃO vai no instalador

Os modelos Live2D em `apps/desktop/src/renderer/public/live2d/` são **amostras de terceiros**,
removidas do bundle pelo passo `closeBundle` descrito acima.

| Pasta   | Modelo                                           | Licença                                                                                                              |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| `haru/` | haru_greeter_t05, amostra oficial da Live2D Inc. | [Live2D Free Material License Agreement](https://www.live2d.com/eula/live2d-free-material-license-agreement_en.html) |
| `Mao/`  | Amostra oficial do Cubism SDK                    | Live2D Free Material License Agreement                                                                               |

A Free Material License permite uso e modificação, mas impõe condições sobre **redistribuição** e
sobre uso comercial acima de um teto de faturamento. Empacotar as amostras num instalador assinado é
redistribuição. Fora do pacote, o desenvolvedor que roda o repositório continua usando as amostras
sob a licença original.

As `motion/*.motion3.json` dentro dessas pastas são autorais deste projeto — geradas por
`pnpm live2d:gen` a partir de `scripts/live2d/action-defs.ts`. A licença acima cobre o `.moc3`, as
texturas e o `physics3.json`, não as motions.

### Como usar Live2D numa instalação

O app abre no VRM por padrão. Para usar um modelo Live2D:

1. Baixe as amostras oficiais em <https://www.live2d.com/en/learn/sample/> e aceite a licença.
2. Coloque cada modelo em `<pasta de dados>/live2d/<nome>/` (no Windows,
   `%APPDATA%/com.mika.desktop/live2d/<nome>/`; é a pasta que o protocolo `mika-live2d` serve).
3. Escolha na aba **Skin** das configurações.

Um modelo Live2D ausente falha com erro nomeado na tela — nunca com um avatar em branco.

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
