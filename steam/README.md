# Mika na Steam — publicação e atualização automática

Dois canais, nunca misturados:

| Canal                  | Build                                                  | Quem atualiza                                                      |
| ---------------------- | ------------------------------------------------------ | ------------------------------------------------------------------ |
| `direct` (site/GitHub) | `tauri build`                                          | `tauri-plugin-updater`, lendo `latest.json` da release             |
| `steam`                | `tauri build --no-bundle --features steam -- --locked` | **cliente Steam** (SteamPipe). O updater do Tauri nem é registrado |

"Atualização automática dentro da Steam" = o CI (`.github/workflows/release.yml`) envia cada release a um
branch Steam com `steamcmd`; o cliente baixa sozinho. `updateChannel` (`steam`/`direct`) vai no health e a UI
troca a aba "Atualizações" por um aviso na build Steam.

## 1. Steamworks (uma vez, no painel de parceiro)

1. **Criar o app** e pagar a taxa; anotar o **AppID**.
2. **Depots** (App Admin > SteamPipe > Depots): um para Windows e um para Linux; anotar os dois **DepotID**.
   Em "Installation > General", launch options: Windows `mika-desktop.exe`; Linux `bin/mika-desktop`.
3. **Branches** (SteamPipe > Builds): criar `beta` (o CI publica aqui por padrão). `default` é produção.
4. **Redistribuíveis**: o `installscript.vdf` do depot Windows instala VC++ 2015-2022 x64 e o WebView2 em
   silêncio (o CI baixa os instaladores da Microsoft para `_CommonRedist/`). Em Installation > Redistributables,
   não marque nada em duplicidade.
5. **Auto-Cloud: NÃO habilitar.** A pasta de dados mistura o que não pode ir para a nuvem da Valve:
   `config.json` (perfil `user.*`, chaves de API cifradas), `master.key`/`*.key`/`secrets*`,
   `plugins/**` (`chat.db` = conversas, `memory.db` = memória, `catalog.json` = pessoas), `exports/`,
   `backups/`, `reports/`, `usage-habits.json`, `captures/` e `logs/`. Deixe Steam Cloud **desligado** (quota 0)
   até existir um arquivo de preferências sem dados pessoais e um opt-in na UI — `PRIVACY.md` (seções 1 e 3)
   promete que nada é sincronizado. Se um dia habilitar, use apenas um subconjunto explícito (nunca `*.json`
   recursivo) e **exclua** tudo o que está listado acima, mais `*.db*` e `models/*`.
6. **Controle**: em Store > Steam Input/Controller, declarar **sem suporte a controle** (a UI é mouse/teclado).
7. **Conquistas** (Stats & Achievements): criar com estes **API Names** exatos (`src/steam.rs`):
   `MIKA_FIRST_CONVERSATION`, `MIKA_FIRST_VOICE`, `MIKA_SEVEN_DAYS`, `MIKA_FIRST_MEMORY`,
   `MIKA_FIRST_GAME_COMMENT`. Publicar as mudanças de stats/conquistas.
8. **Rich Presence** (Community > Rich Presence): token `"#Status" "%status%"` (inglês). A Mika define
   `steam_display=#Status` e `status="Conversando com a Mika"`.
9. **Questionário de conteúdo gerado por IA**: declarar texto, voz e visão gerados ao vivo (LLM local
   ou provedor remoto do usuário) e o modo "relationship" da persona. **Só descreva guardrails que existem
   no código** — hoje: (a) piso de segurança no prompt (`prompts/brain/system.md`, "Safety floor", repetido em
   `system-tail.md`), acima de persona e `customInstructions`: crise/autolesão, menores, conteúdo sexual,
   dano, dependência; (b) detector determinístico de crise nos 10 idiomas (`modules/brain/crisis.rs`) que
   injeta a nota de crise no turno e mostra o cartão de serviços de apoio; (c) aceite com "18 anos ou
   mais" (`legal.ageConfirmed`), sem o qual o vínculo `companion` cai para `friend`; (d) botão "Reportar esta
   resposta" (relatório **local**, nada é enviado); (e) aviso de conteúdo gerado por IA no aceite;
   (f) rótulo de conteúdo não confiável contra injeção de prompt. **Não afirme:** moderação de saída por
   classificador, verificação de idade, filtro de conteúdo do provedor remoto, nem canal de reporte à
   desenvolvedora (falta o e-mail de suporte). Responder também o IARC e a página de privacidade.
10. **Publicar** as alterações do app (Publish) e dar à conta de build a permissão "Edit App Metadata" e
    "Publish App Changes" **apenas** neste app.

## 2. Conta de build e secrets do GitHub

Crie uma conta Steam **só para o CI**, com Steam Guard no app móvel, e extraia o `shared_secret`
(base64) do autenticador (ex.: `steamguard-cli`/SDA). Em Settings > Secrets and variables > Actions:

| Secret                                            | Valor                                                                    |
| ------------------------------------------------- | ------------------------------------------------------------------------ |
| `STEAM_USERNAME` / `STEAM_PASSWORD`               | conta de build                                                           |
| `STEAM_TOTP`                                      | `shared_secret` em base64 (o CI gera o código com `steam/guard-code.py`) |
| `STEAM_APP_ID`                                    | AppID (também entra no binário: `MIKA_STEAM_APP_ID`)                     |
| `STEAM_DEPOT_WINDOWS_ID` / `STEAM_DEPOT_LINUX_ID` | os dois DepotID                                                          |

Sem esses secrets (forks, por exemplo) o canal Steam simplesmente não roda. O job só existe em
`juninmd/mika`.

Disparo: todo push em `main` com versão nova envia ao branch **`beta`**. Manual: _Actions > Release > Run
workflow_ com `steam_branch` (qualquer nome de branch). **`default` só com `confirm_default` marcado.**
`steam_preview` valida os scripts sem enviar. A Steam pode recusar `SetLive` em `default`; nesse caso o build
fica enviado e é promovido em Steamworks > Builds.

- **Só a `main` publica.** Dispatch em qualquer outro branch roda a validação e desliga `direct` e `steam`.
- **Promover beta -> default:** dispatch na `main` com `steam_branch=default` e `confirm_default`. Sem commits
  novos desde a última tag, o workflow republica essa versão (só Steam; a release do GitHub não é refeita).
- **Ambientes do GitHub** (Settings > Environments): `steam-production` (usado quando `steam_branch=default`)
  e `release` (job `direct`, o único que enxerga a chave de assinatura do updater) devem ter **revisores
  obrigatórios**; `steam-beta` é criado sem proteção. Os secrets podem continuar no nível do repositório.
- **Tag `vX.Y.Z`:** nasce no job `tag`, depois de algum canal ter entregue (a `publish` do GitHub também a
  cria). Sem entrega, nenhuma tag — o próximo push recalcula a mesma versão.
- **Log do steamcmd:** o passo falha se o log não tiver `Successfully finished AppID`, e o log (com o login
  mascarado) sobe como artefato `steam-build-output`, mesmo em falha.
- **steamcmd:** o tarball é conferido por SHA-256 fixo no workflow. Se a Valve republicar o arquivo, o passo
  falha com o hash novo; confira a origem e atualize o hash.

Teste local do depot: `pnpm --filter @mika/desktop exec tauri build --no-bundle --features steam -- --locked` e
depois `bash steam/assemble-depot.sh windows steam-depot/windows` (ou `linux`).

### Layout do depot e por que

`bundle.resources` (`prompts/` e `legal/`) é copiado para `target/release/` pelo `build.rs` do `tauri-build`
mesmo com `--no-bundle`; o que o `--no-bundle` pula é o bundler, que poria esses arquivos no pacote final. O
`assemble-depot.sh` os leva ao lugar onde o Tauri os procura (`resource_dir`):

| SO      | Executável         | Recursos (`prompts/`, `legal/`)                                                             |
| ------- | ------------------ | ------------------------------------------------------------------------------------------- |
| Windows | `mika-desktop.exe` | ao lado do exe                                                                              |
| Linux   | `bin/mika-desktop` | `lib/Mika/` — `<exe>/../lib/<productName>`; é o `productName` ("Mika"), não o nome do crate |

`legal/` (EULA, privacidade, notices) é lido pelo app em runtime (`modules/legal.rs`, `BaseDirectory::Resource`);
sem ele a tela Sobre/aceite do EULA falha.

## 3. Auto-update do canal `direct`

No app, `app.autoUpdate` (Configurações → Atualizações) decide o que o laço de fundo faz
(`apps/desktop/src-tauri/src/modules/update/auto.rs`): `install` (padrão) checa a cada 6 h, baixa fora de jogo
e de bateria e instala só com o PC ocioso; `notify` só checa e avisa; `off` deixa só o botão manual. O laço
não nasce na build Steam nem sem endpoint de update. Nada disso funciona sem uma release com `latest.json`:

Gerar o par de chaves (uma vez) e guardar a chave privada com segurança:

```
pnpm --filter @mika/desktop exec tauri signer generate -w ~/.tauri/mika.key
```

- Secret `TAURI_SIGNING_PRIVATE_KEY` = conteúdo do `.key`; `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = senha.
- Variável (não secret) `TAURI_UPDATER_PUBKEY` = conteúdo do `.key.pub`. O `tauri.conf.json` **não** leva
  pubkey: o CI injeta com `--config`, junto com `createUpdaterArtifacts` e o endpoint
  `https://github.com/<repo>/releases/latest/download/latest.json`.
- **Sem esse par o job `direct` não roda e nada é publicado**: uma release sem `latest.json` quebraria a
  checagem de quem já instalou. Perder a chave privada = os apps instalados nunca mais aceitam update.
- A chave existe só no passo "Empacotar" do job `direct` (ambiente `release`), que instala dependências com
  `--ignore-scripts` (só o `esbuild` é reconstruído, de forma explícita).
- Instalador Windows: o bundler só empacota exe + `bundle.resources`, então o `DirectML.dll` que o `ort` deixa
  em `target/release/` entra por `apps/desktop/src-tauri/installer-hooks.nsh` (lê `MIKA_DIRECTML_DLL`, definido
  pelo CI; o CI confere com `7z l` que a DLL está no instalador). Sem a DLL o ONNX Runtime cai para CPU, com
  `log::warn!` em `modules/inference/session.rs`. O CRT do exe é estático (`build.windows.staticVCRuntime`,
  padrão `true`); `bundleVCRuntime` (injetado pelo CI) leva as DLLs do VC++ ao lado dele para DLLs de terceiros.
- Opcionais: `WINDOWS_CERTIFICATE` (PFX em base64) + `WINDOWS_CERTIFICATE_PASSWORD` para Authenticode;
  `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`,
  `APPLE_TEAM_ID` para assinar e notarizar o dmg. Sem eles os instaladores saem sem assinatura.

## 4. Riscos a testar antes do lançamento

- **Linux/Steam Runtime**: o app usa WebKitGTK (`libwebkit2gtk-4.1`), que o runtime "sniper" da Steam não
  traz, e o binário é compilado no `ubuntu-24.04` (glibc 2.39, mais nova que a do sniper, 2.31; o `xcap`/`pipewire 0.10` exige
  headers do PipeWire >= 1.0, que o 22.04 não tem). Logo o depot Linux só roda **no host** (Ubuntu >= 24.04,
  SteamOS 3.7+ com webkit instalado; confirme a glibc do SteamOS no Deck): em Steamworks, deixe a launch
  option Linux **sem** "Steam Linux Runtime" e não marque o container sniper. Teste no SteamOS/Steam Deck e em
  Ubuntu; se faltar `libwebkit2gtk-4.1`, o depot Linux fica fora até haver solução (bundlar o WebKit ou
  compilar contra o sniper).
- Janela transparente do avatar no Wayland/Steam Deck. O app define `WEBKIT_DISABLE_DMABUF_RENDERER=1` no
  Linux (se você não definiu) para evitar a janela branca do renderer DMA-BUF; `lib.rs::run`.
- Primeiro build `beta`: conferir no painel que o depot Windows contém `installscript.vdf` na raiz e que o
  `exe` abre a partir da Steam (fora dela, a build de produção se relança pelo cliente).
- Documentos legais (EULA, privacidade, notices de terceiros) são pré-requisito da página da loja; o
  `assemble-depot.sh` leva `legal/` (obrigatório, o app lê) e copia `LICENSE*` e `THIRD-PARTY-NOTICES*` se
  existirem na raiz.
- Redistribuíveis do depot Windows (`vc_redist.x64.exe`, bootstrapper do WebView2): URLs móveis da Microsoft,
  sem hash fixo possível; o CI exige assinatura Authenticode válida da Microsoft (`Get-AuthenticodeSignature`).
