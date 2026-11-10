# 02 — Provedores

**Mecanismo pós-Tauri (2026-09-24):** o catálogo (`LLM_PROVIDER_IDS`, `LLM_PROVIDER_CATALOG`) continua
em TypeScript, `packages/sdk/src/llm/llm-catalog-data.ts` (`packages/llm` — o resolver de chat — foi
apagado em 2026-09-23; só o catálogo, que é dado, sobreviveu no SDK). O backend tem dois pontos Rust
que espelham essa mesma lista de 5 ids como literal (não importam o símbolo TS, ver comentário de
cabeçalho de cada um):
- `apps/desktop/src-tauri/src/modules/llm.rs` — só o **catálogo de modelos** (listar/pesquisar), saúde
  e quota (5 canais IPC); o cabeçalho do próprio arquivo ainda diz "chat/streaming fica no sidecar
  Node" — **esse comentário está desatualizado**, ver o item abaixo.
- `apps/desktop/src-tauri/src/modules/brain/llm_provider.rs` + `apps/desktop/src-tauri/src/modules/brain/turn.rs`
  — a **chamada de chat de verdade**, para os 5 provedores, via `rig-core`. Roda inteira em Rust desde
  a onda `6b9bb6ec` (`brain::turn::run_turn_llm`); não existe mais sidecar Node nenhum no app
  (`packages/llm` apagado, `check:sidecar-attrition` em 0 — ver `AGENTS.md`).

## Os cinco

O mesmo conjunto vale para **chat, visão e embeddings**. Sem exceção por superfície.

| id                 | Nome               | Credencial                  | Catálogo de modelos | Local?             |
| ------------------ | ------------------ | --------------------------- | ------------------- | ------------------ |
| `google`           | Google AI (Gemini) | API key                     | `v1beta/models`     | não                |
| `openrouter`       | OpenRouter         | API key                     | `/api/v1/models`    | não                |
| `ollama`           | Ollama             | base URL + **key opcional** | `/api/tags`         | local **ou** cloud |
| `openaiCompatible` | OpenAI Compatible  | base URL + key opcional     | `{baseUrl}/models`  | depende            |
| `unsloth`          | Unsloth (Studio/Desktop) | base URL (padrão `http://localhost:8888/v1`) + key `sk-unsloth-…` | `{baseUrl}/models` (o que está carregado) | local |

`openaiCompatible` cobre LiteLLM, vLLM, NVIDIA NIM, **a própria OpenAI** (`https://api.openai.com/v1`)
e o Ollama pelo `/v1` dele.

**MUST NOT** existir um provedor `openai` nativo separado.

> **Por quê:** era o mesmo protocolo com resolver dedicado, chave de config própria e catálogo
> estático próprio — três lugares para divergir, zero capacidade a mais.

**`unsloth` é a exceção deliberada, e não reabre o defeito acima.** Ele fala o MESMO protocolo
(`/v1/chat/completions`, `/v1/models`, `/v1/embeddings`; `unsloth.ai/docs/basics/api`) e por isso
reusa o resolver do `openaiCompatible` — um cliente `rig-core`, um catálogo vivo, uma guarda de
endpoint (`modules/llm.rs::OpenAiShaped`/`openai_shaped`, a tabela que diz qual caminho de config
cada um lê). O que ele tem de PRÓPRIO é o que o usuário não pode compartilhar sem vazar credencial
ou apontar para o servidor errado: `apiKeys.unslothBaseUrl` (padrão `http://localhost:8888/v1`),
`apiKeys.unslothApiKey` (`sk-unsloth-…`, criada em Settings → API do Unsloth e exibida uma única
vez) e `llm.unslothModel`. **Sem modelo de fábrica**: o id é o que o Unsloth tem carregado
(`GET /v1/models`), então o catálogo é sempre o vivo e um turno sem modelo escolhido falha NOMEADO
(`unsloth: modelo não configurado`), nunca com um nome inventado. **Só `top_p`** vai no corpo da
requisição: `chat_template_kwargs`/`id_slot` são dialeto do llama-server cru, e o Unsloth fica na
frente dele sem documentar que repassa campo desconhecido — não mando o que não verifiquei. O
embedding também não tem modelo padrão (diferente do `text-embedding-3-small` do
`openaiCompatible`). A chave `sk-unsloth-…` é opcional para o app (o servidor responde 401 com o
motivo, que sobe ao usuário), mas a doc do Unsloth a exige em toda requisição.

**MUST NOT** existir o provedor embarcado `local` (transformers.js). Ele saiu do catálogo, das
settings e do código junto com `packages/llm-local` e `packages/embedding-local`: o runtime ONNX
pesava no boot, duplicava o host de inferência e o offline-first já é coberto pelo Ollama.

## O provedor de fábrica é o Ollama

`ollama` é o default de fábrica de `llm.provider` e `llm.embeddingProvider`: instalou, a Mika fala —
modelo local servido pelo usuário, sem chave. Nuvem é escolha explícita do usuário
(wizard/Settings), nunca pré-requisito.

> **Por quê:** o default histórico era o runtime embarcado (`local`), que baixava pesos do
> Hugging Face Hub no primeiro uso e rodava 100% no processo — a mesma lição do Kokoro: ONNX
> CPU-bound não divide processo com a UI. As rotas "locais" restantes (Ollama, LM Studio via
> openaiCompatible) exigem o usuário subir um servidor por fora; sem elas, o app de fábrica só
> funcionava com chave de nuvem.

**MUST NOT**: a mudança de default reconfigurar instalações existentes — `applyDefaults` persiste
o valor no primeiro boot, então quem já rodava com `google` gravado continua em `google`.

**MUST NOT**: os hosts de processo próprio (`inference-host`) ter fallback para o processo main.
Host fora do ar → o trabalho é **rejeitado com erro visível** (estado DEGRADADO, logado uma vez).
"Fala que não sai" é um erro que o usuário vê; UI travada é o app inteiro parecendo morto — entre
os dois, o erro visível vence.

**MUST**: `ollama` é o alvo final do failover ([03](03-resilience.md)) — sempre elegível (base URL
basta, credencial é opcional) e sempre gratuito (roda na máquina do usuário).

**[Portado nesta rodada, com escopo]** `brain::turn::run_turn_llm` agora tenta Ollama automaticamente
quando o provedor remoto configurado falha **ANTES de qualquer efeito colateral** neste turno
(`TurnError::ProviderBeforeSideEffects` — nenhum texto emitido, nenhuma tool executada; cobre o caso
comum do incidente de referência, credencial/modelo inválido detectado na primeira chamada). Uma
falha DEPOIS de efeito colateral (`TurnError::Provider` — texto já streamado ao usuário e/ou tool já
executada) nunca tenta de novo: refazer o turno inteiro duplicaria a tool. Se o próprio failover para
Ollama também falhar, o erro devolvido é o do provedor ORIGINAL (não o do Ollama de socorro). A falha
do provedor configurado é gravada em `provider_health` mesmo quando o failover TEM sucesso
(`TurnResult::served_by_failover`) — sem isso, o registro de saúde "esqueceria" que o par configurado
está quebrado só porque o usuário recebeu uma resposta (ver [03](03-resilience.md)). Não portado:
failover para visão/embeddings, e failover encadeado entre os 3 provedores remotos (só remoto→ollama,
nunca remoto→remoto).

## Cloud ou local é escolha do usuário, não do código

**MUST**: qualquer superfície (chat, visão, embeddings) aceita qualquer um dos cinco. O código
**MUST NOT** assumir que uma capacidade só funciona na nuvem.

E a recíproca, que é o erro fácil de cometer:

**MUST NOT** assumir que um provedor "local" dispensa credencial. A presença de
`credentialConfigKey` no catálogo é o que decide — nunca inferida de URL nem de nome.

O Ollama roda em três cenários, e os três são de primeira classe:

| Cenário                          | Base URL                 | Key              |
| -------------------------------- | ------------------------ | ---------------- |
| Local                            | `http://localhost:11434` | nenhuma          |
| **Ollama Cloud**                 | `https://ollama.com`     | **obrigatória**  |
| Self-hosted atrás de proxy/túnel | URL própria              | conforme o proxy |

**Guarda de endpoint (Rust).** Endereço DIGITADO pelo usuário — base URL de Ollama/`openaiCompatible`
e do TTS OpenAI — passa por `privacy::assert_user_endpoint_allowed` (`modules/privacy/endpoint_guard.rs`),
porte do `validateUrl` do TS: loopback passa; o resto só com http(s) e IP público, e um nome de host
só se TODOS os endereços resolvidos forem públicos (DNS que falha bloqueia). Rede privada (LAN,
`169.254.x`, `fc00::/7`) continua recusada, como no TS. Depois vem sempre o gate de modo offline.
- **Chat Ollama** e **catálogo com o endereço já salvo** (`assert_ollama_catalog_endpoint_allowed`)
  confiam no endereço que o próprio usuário salvou: só o gate de modo offline (Ollama Cloud incluso).
- **Sondagem de endereço ainda não salvo** (tela de configuração) e **`openaiCompatible`** (chat e
  catálogo) passam pela guarda completa; `key_for_endpoint` garante que a sondagem nunca leva a
  credencial salva. Testes: `probed_endpoint_different_from_stored_goes_through_ssrf_then_offline`
  (`modules/llm.rs`) e `privacy::endpoint_guard::tests`.
- Limite herdado do TS: resolve e depois conecta, então DNS rebinding entre as duas etapas passa.

**MUST**: `ollama` tem **base URL e API key como campos separados**, e a key é opcional.

**MUST**: quando a key existe, ela vai como `Authorization: Bearer` em **toda** chamada ao
provedor — geração, embeddings e listagem de catálogo. Listar modelos sem o header em cloud
devolve catálogo vazio ou 401, e o sintoma que chega ao usuário é "a lista de modelos não
aparece".

> **Por quê:** tratar Ollama como "o local, sem credencial" fez a base URL ser guardada **no lugar
> da chave** — o acessor de credencial do provedor devolvia a URL. Funciona por acidente enquanto
> só existe localhost, e torna cloud impossível de configurar sem refatorar.

**MUST NOT** derivar cloud-vs-local do formato da URL. Presença de key é configuração explícita,
não inferência — mesma regra de [capacidade de modelo](#capacidade-de-modelo-reporte-não-adivinhe).

## Catálogo de modelos vem do provedor, sempre

**MUST**: a lista de modelos oferecida ao usuário vem de chamada real à API do provedor.

**MUST NOT** existir lista estática de modelos como fonte de verdade. Lista embutida serve apenas
como fallback quando não há credencial configurada, e **MUST** ser marcada `source: 'static'`.

> **Por quê:** Gemini, OpenAI e compatible devolviam lista hardcoded. O usuário não conseguia
> escolher modelo nenhum que não estivesse nos 3 embutidos, e por isso digitava o id na mão — foi
> exatamente assim que uma tag do Ollama (`gemma3:4b`) acabou apontada para o OpenRouter.

## Capacidade de modelo: reporte, não adivinhe

**MUST NOT** inferir capacidade (visão, ferramentas) a partir do nome ou id do modelo.

Só o OpenRouter reporta capacidade de verdade, via `architecture.input_modalities`. Para os
demais, `supportsVision` e `supportsTools` ficam **indefinidos**.

**MUST**: quando o provedor não reporta, a UI mostra o catálogo completo e **diz** que aquele
provedor não informa a capacidade. Filtrar para zero é pior do que não filtrar.

> **Por quê:** um regex de nomes (`/gpt-4o|vision|llava|qwen.*vl/`) mente com confiança. Modelo
> novo cai fora, modelo com nome parecido entra errado, e o usuário não tem como saber que a
> resposta veio de um chute.

## Schema não pode ser mais estreito que o catálogo

**MUST**: campos de modelo em schema de config são `z.string()`.

**MUST NOT** usar `z.enum([...])` com lista de modelos.

> **Por quê:** `llm.googleModel` era `z.enum(['gemini-2.5-flash','gemini-2.5-flash-lite','gemini-1.5-flash'])`.
> Qualquer Gemini novo escolhido na UI era **rejeitado na gravação**. E o `LLMFactory` tinha o
> mesmo problema pelo outro lado: tratava google como "provedor estático", validava contra a mesma
> lista e trocava pelo default **sem log e sem aviso**.

Corolário: **MUST NOT** haver fallback silencioso de modelo. Modelo inválido é responsabilidade do
circuit breaker ([03](03-resilience.md)), que avisa na UI.

## Enums de provedor derivam da fonte única

**MUST**: todo `z.enum` de provedor usa `LLM_PROVIDER_IDS`, nunca uma lista literal repetida.

> **Por quê:** o schema de embeddings tinha `['google','openai','ollama','openaiCompatible','local']`
> — **sem `openrouter`**. O resolver suportava OpenRouter desde sempre; a validação barrava. Uma
> lista literal copiada é uma divergência esperando a data.

Exceções legítimas ao conjunto de 5, que **MUST** ser documentadas no próprio schema:
- Visão aceita `default` (herda o provedor do chat).

**[Feature adicionada]** O schema de config espelhado em Rust
(`apps/desktop/src-tauri/src/config/schemas/llm.rs::PROVIDER_IDS`,
`apps/desktop/src-tauri/src/config/schemas/vision.rs`) não pode importar o símbolo TypeScript
`LLM_PROVIDER_IDS` — é outra linguagem — e copia a lista como `&[&str]` literal, com comentário
explícito apontando `llm-catalog-data.ts` como fonte. Isso não viola o espírito do MUST (fonte única
DENTRO de cada linguagem), mas é uma segunda cópia que só o comentário mantém sincronizada; não há
gate encontrado que compare literalmente os dois lados.

## Credenciais em um lugar só

**MUST**: todas as chaves e URLs vivem numa aba única de Credenciais, agrupadas por domínio
(LLM, TTS, STT, canais, integrações, infra).

**MUST NOT** um painel de feature pedir credencial que já existe na aba central.

**MUST NOT** logar chave, nem prefixo, nem tamanho. Só `apiKeyPresent: boolean`.

> **Por quê:** os primeiros 8 caracteres já identificam a conta em vários provedores, e log vai
> para arquivo.

## Seleção de modelo na UI

**MUST**: seleção de modelo é feita por seletor alimentado pelo catálogo, com busca.

**MUST NOT** existir campo de texto livre para id de modelo.

O seletor **MUST**:
- mostrar o valor salvo mesmo quando ele não está no catálogo, com aviso explícito de que não
  pertence àquele provedor;
- exibir capacidade (visão, ferramentas), preço/gratuito e janela de contexto quando o provedor
  informar;
- permitir recarregar o catálogo;
- ao filtrar por capacidade, oferecer escape ("mostrar todos") com a contagem do que foi ocultado.
