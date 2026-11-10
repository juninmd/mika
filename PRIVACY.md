# Política de Privacidade — Mika

> **RASCUNHO — requer revisão jurídica antes da publicação.**
> Cada afirmação abaixo foi conferida no código desta versão do Mika. Campos entre colchetes
> (`[RAZÃO SOCIAL]`, `[E-MAIL DE SUPORTE]`, `[ENCARREGADO (DPO)]`) são placeholders.
> Vigência: `[DATA DE VIGÊNCIA]`.

## 1. Resumo

- O Mika roda **no seu computador**. **Não há conta, login nem servidor nosso** que receba as suas
  conversas, a sua voz ou a sua tela.
- Seus dados ficam em uma pasta local (seção 2). Eles só saem do computador quando **você** liga um
  recurso que fala com um serviço externo (seção 3) ou aciona uma ação que depende dele.
- **Nesta versão, nenhum dado de uso, de erro ou de telemetria é enviado a nós ou a quem quer que
  seja** (seção 4). Não há SDK de análise nem de relatório de falhas no aplicativo.
- O modo offline (seção 5) bloqueia toda comunicação externa.
- **Steam Cloud:** o Mika **não** sincroniza dados pela Steam Cloud. Seus dados ficam só na pasta local
  (seção 2); se uma versão futura sincronizar algo, será opt-in e esta política dirá o quê.
- Tudo o que o Mika diz é **gerado ao vivo por IA** (EULA, seção 3) e pode estar errado.

Controlador: **[RAZÃO SOCIAL]**, [ENDEREÇO]. Contato: **[E-MAIL DE SUPORTE]**. Encarregado/DPO:
**[ENCARREGADO (DPO)]**. Para os dados que ficam só no seu computador, não temos acesso a eles; para
os que você envia a um provedor de IA, o provedor trata o dado sob a política dele.

## 2. O que fica no seu computador

Pasta de dados: `%APPDATA%/com.mika.desktop` no Windows (`~/.local/share/com.mika.desktop` no Linux,
`~/Library/Application Support/com.mika.desktop` no macOS). Dentro dela:

| O que                                                  | Arquivo ou pasta                                                                                                    | Observação                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Configurações                                          | `config.json`                                                                                                       | Chaves de API são cifradas (AES-256-GCM) com uma chave mestra guardada em `master.key` **na mesma pasta**: isso impede a leitura casual, não protege contra quem tem acesso à sua conta do sistema.                                                                                                                                                                                                                                                                                                           |
| Histórico de conversa                                  | `chat.db`                                                                                                           | Retenção configurável (seção 7).                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Memória do Mika (fatos e episódios) e vetores de busca | `memory.db`                                                                                                         | Pode incluir fatos que o Mika extrai da conversa e da tela.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Seu perfil                                             | dentro de `config.json`                                                                                             | Só o que você preenche: nome, apelido, nascimento, cidade, profissão, hobbies, parceiro(a) e similares. Tudo é opcional.                                                                                                                                                                                                                                                                                                                                                                                      |
| Catálogo de pessoas                                    | `plugins/mika.person-catalog`                                                                                       | Pessoas citadas na conversa (nome, relação, biografia, fotos).                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Lembretes                                              | `scheduler.db`                                                                                                      |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Hábitos de uso                                         | `usage-habits.json`                                                                                                 | **Ligado por padrão** (Humor → "Hábitos de uso"): a cada minuto de uso, soma o tempo ao **nome do processo** em primeiro plano (nunca o título), por dia, com poda de dias antigos. Não conta com você ausente, com o watcher de janela parado nem em janelas da lista de bloqueio. Fica só aqui; o LLM lê o top de programas.                                                                                                                                                                                |
| Cópias antes de atualizar                              | `backups/pre-<versão>/`                                                                                             | Antes de rodar um instalador, o Mika copia chat, memória, agenda, sessões de jogo, uso de tokens, catálogo de pessoas e config; guarda as duas últimas.                                                                                                                                                                                                                                                                                                                                                       |
| Relatórios de resposta                                 | `reports/<timestamp>.json`                                                                                          | Criados só quando você clica em "Reportar esta resposta": o texto da resposta, o modelo e a persona. Sem tela, sem áudio, sem a sua mensagem. **Nada é enviado**: o arquivo é seu para mostrar a quem quiser.                                                                                                                                                                                                                                                                                                 |
| Sessões de jogo e metas                                | `gamer-sessions.db`                                                                                                 |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Contadores de uso de tokens                            | `token_usage.db`                                                                                                    | Somas agregadas por provedor e modelo. Ficam só aqui.                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Registros de funcionamento (logs)                      | pasta `logs`: `mika-human.log` (eventos que importam à pessoa) e `mika-ai.ndjson` (tudo, uma linha JSON por evento) | Máximo de 10 MiB por arquivo (acima disso, o arquivo é truncado, sem histórico). Antes de gravar, o Mika redige chaves de API conhecidas (`sk-…`, `sk-or-v1-…`, `AIza…`, `gsk_…`), tokens `Bearer`, cabeçalhos `Authorization`, JWT e pares `chave=`/`token=`/`senha=`; a redação é por padrão de texto, então uma credencial de formato desconhecido em uma mensagem de erro pode escapar. Podem conter trechos de conteúdo (por exemplo, título de janela e erros do provedor). Ficam só no seu computador. |
| Exportações                                            | pasta `exports`                                                                                                     | Geradas só quando você pede. O `.zip` **não tem senha** nesta versão: guarde-o em disco cifrado ou apague-o depois de usar.                                                                                                                                                                                                                                                                                                                                                                                   |
| Modelos baixados                                       | `whisper-cache`, `nemotron-cache`, `vad-cache`, `e5-cache`, `supertonic`                                            | Arquivos de modelo; não contêm dados seus.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Avatares Live2D que você instalar                      | pasta `live2d`                                                                                                      |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

A detecção de sinais de crise (autolesão, suicídio) na sua mensagem roda **no seu computador**, por uma lista fixa de frases: nada é gravado, enviado nem contado; ela só acrescenta uma instrução ao pedido do turno e mostra um cartão com serviços de apoio.

Capturas de tela **não são gravadas em disco**: a imagem vai da tela para o provedor de visão que você
configurou e é descartada (a pasta `captures`, se existir, só é apagada pelo "apagar meus dados").

O Mika lê localmente o nome do processo e o título da janela em foco para detectar jogos e contexto
(e-mails, telefones, CPF/CNPJ, cartões e tokens são redigidos antes do uso). Esse contexto pode entrar
no pedido enviado ao modelo de linguagem configurado.

## 3. O que sai do computador, e quando

Nada abaixo acontece sem que o recurso esteja ligado ou que você peça a ação. "Padrão" é o estado da
instalação nova.

| Recurso                                   | Para onde                                                                                                                                                                                                                                          | O que é enviado                                                                                                                                                                         | Padrão                                                                                                                                                                                                        |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Modelo de linguagem (chat)**            | Ollama no seu computador (`localhost`) por padrão. Se você escolher: Google Gemini (`generativelanguage.googleapis.com`), OpenRouter (`openrouter.ai`), um endpoint compatível com OpenAI (o endereço que você digitar) ou um Ollama em outro host | Suas mensagens, o histórico recente, trechos da memória, os campos do perfil usados pela persona, resultados de ferramentas e, se a visão usar um provedor remoto, imagens da tela      | **Local**                                                                                                                                                                                                     |
| **Embeddings da memória**                 | Local por padrão. Se você escolher Google, OpenRouter, Ollama ou endpoint compatível: o provedor escolhido                                                                                                                                         | Os textos que viram vetores                                                                                                                                                             | **Local**                                                                                                                                                                                                     |
| **Reconhecimento de fala (STT)**          | Local (Nemotron/Whisper) por padrão. Se você escolher: Groq (`api.groq.com`) ou OpenAI (`api.openai.com`)                                                                                                                                          | O áudio das suas falas                                                                                                                                                                  | **Local**                                                                                                                                                                                                     |
| **Reconhecimento "Browser (Web Speech)"** | Serviço de fala do componente de navegador do sistema, cujo funcionamento depende do fornecedor dele                                                                                                                                               | Áudio, conforme esse serviço                                                                                                                                                            | Não é o padrão; evite se quiser fala 100% local                                                                                                                                                               |
| **Voz (TTS)**                             | Local (Supertonic) por padrão. Se você escolher: ElevenLabs (`api.elevenlabs.io`), Azure (`*.tts.speech.microsoft.com`), Google (`texttospeech.googleapis.com`) ou um endpoint compatível com OpenAI                                               | O texto das respostas do Mika                                                                                                                                                           | **Local**                                                                                                                                                                                                     |
| **Visão (tela)**                          | O provedor de visão configurado (local ou remoto)                                                                                                                                                                                                  | Imagens da tela                                                                                                                                                                         | **Desligada**; exige "Visão" **e** "Captura de tela"                                                                                                                                                          |
| **Busca na web**                          | Bing, DuckDuckGo ou Yahoo (você escolhe, ou "automático"); páginas abertas pelo Mika passam por um filtro contra acesso a rede interna                                                                                                             | O texto da busca, inclusive o nome do jogo detectado quando o Mika pesquisa sobre ele; o IP chega ao site                                                                               | **Desligada** (opt-in em Navegador → "Permitir busca na web"; vale para perfis novos — quem já tinha o `config.json` mantém o valor gravado)                                                                  |
| **Enriquecimento do catálogo de pessoas** | Wikipédia (`en.wikipedia.org`, `upload.wikimedia.org`) e os motores de busca acima                                                                                                                                                                 | O **nome** de figuras públicas, personagens, jogos, filmes etc. recém-adicionados ao catálogo. Pessoas com quem você tem relação (parente, chefe...) **nunca** são pesquisadas          | **Desligado** (opt-in em Catálogo de pessoas → "Consultar novas pessoas na web", independente da busca na web; o modo offline também o bloqueia; perfis que já tinham o `config.json` mantêm o valor gravado) |
| **Steam Web API**                         | `api.steampowered.com`                                                                                                                                                                                                                             | Sua chave da Steam e seu SteamID64, para listar jogos e conquistas                                                                                                                      | Só se você informar a chave                                                                                                                                                                                   |
| **Testar credencial**                     | O provedor da chave testada                                                                                                                                                                                                                        | A chave, em uma consulta de lista                                                                                                                                                       | Só ao clicar em "Testar"                                                                                                                                                                                      |
| **Download de modelos**                   | `huggingface.co` e `raw.githubusercontent.com`                                                                                                                                                                                                     | Nenhum dado seu; o servidor vê o seu IP. Os arquivos vêm de versões fixas e são verificados por hash quando há hash publicado                                                           | Ao ligar o recurso ou no assistente de configuração                                                                                                                                                           |
| **Servidores MCP**                        | O que cada servidor fizer: são programas de terceiros no seu computador, com rede própria                                                                                                                                                          | O que o servidor receber nas chamadas que você aprovou                                                                                                                                  | **Desligado**                                                                                                                                                                                                 |
| **Ollama: baixar modelo**                 | O Ollama que você configurou                                                                                                                                                                                                                       | O nome do modelo                                                                                                                                                                        | Só quando você pede                                                                                                                                                                                           |
| **Atualização**                           | Steam: pelo cliente Steam. Outras distribuições: nesta versão **nenhum endereço de atualização está configurado**, então nada é consultado                                                                                                         | —                                                                                                                                                                                       | —                                                                                                                                                                                                             |
| **Integração Steam (build Steam)**        | Cliente Steam (Rich Presence, conquistas), sob a política da Valve                                                                                                                                                                                 | Estado do jogo/conquistas                                                                                                                                                               | Só no build Steam                                                                                                                                                                                             |
| **Steam Cloud**                           | —                                                                                                                                                                                                                                                  | **Nada.** O Auto-Cloud da Steam não está habilitado para este app (`steam/README.md`): `config.json` guarda seu perfil e chaves cifradas, e os bancos guardam suas conversas e memórias | Desligado                                                                                                                                                                                                     |
| **Links**                                 | O seu navegador, ao clicar em um link permitido                                                                                                                                                                                                    | —                                                                                                                                                                                       | Ao clicar                                                                                                                                                                                                     |

O que o provedor faz com o dado depende dele. **Só no OpenRouter** o Mika pede retenção zero: toda
requisição de chat e de visão leva `provider: { zdr: true, data_collection: "deny" }`, que restringe o
roteamento a endpoints com retenção zero de dados e que não coletam o conteúdo (o catálogo de modelos
também só lista modelos assim; é um pedido ao OpenRouter, que o aplica sob os termos dele). Os
embeddings pelo OpenRouter e os demais provedores (Google, endpoint compatível com OpenAI, Groq,
ElevenLabs etc.) **não** recebem esse pedido: se isso importa para você, escolha um provedor cujos
termos garantam e use o Ollama local.

## 4. Telemetria

- Existe uma janela de consentimento de telemetria (desligada por padrão: `telemetry.enabled = false`).
  Sua escolha e um identificador de sessão aleatório (UUID) são gravados **só localmente**. O texto da
  janela diz o mesmo: hoje nada é coletado nem enviado, e a opção ainda não muda nada (decisão desta
  versão: a janela descreve o que o código faz, não eventos futuros; quando houver coleta, o texto
  passa a listá-la).
- **Nesta versão não existe código que registre ou transmita eventos de telemetria.** Ligar ou
  desligar a opção não muda o que é coletado: nada é enviado.
- Os únicos contadores mantidos são os de tokens (`token_usage.db`), locais, sempre ativos,
  agregados (sem conteúdo de mensagem). Você pode zerá-los nas configurações.
- Se uma versão futura enviar telemetria, ela continuará **opt-in**, terá o conteúdo descrito aqui e
  esta política será atualizada antes.

## 5. Modo offline

Em Privacidade → Modo offline (ou com a variável `MIKA_BLOCK_EXTERNAL_HTTP=1`), o Mika bloqueia toda
requisição de rede que não seja para o próprio computador (`localhost`) e desconecta os servidores MCP.
Com o Ollama local, voz local e reconhecimento local, o Mika funciona sem enviar nada à internet. O
download de modelos exige rede: baixe-os antes.

## 6. Segurança

Chaves de API cifradas no disco (aviso de limite na seção 2); chaves e tokens redigidos dos logs;
filtro contra acesso a endereços internos nas requisições a endereços digitados por você; ações
sensíveis (controle do computador, execução de código, ferramentas MCP, apagar dados) exigem a sua
aprovação; a bandeja tem "Pausar mic e tela", que suspende o processamento de áudio e a captura de tela até você retomar (o microfone do sistema segue aberto, mas nada que ele capta é processado ou enviado); a lista de bloqueio de captura cobre, por padrão, senhas, bancos, e-mail, mensageiros, navegação privada, carteiras cripto, saúde e autenticadores; o controle do computador, a busca na web e a consulta de pessoas na web **vêm desligados**
em perfis novos. Nenhum software é totalmente seguro; use a proteção do seu sistema (conta com senha,
disco cifrado).

## 7. Retenção

| Dado                        | Quanto tempo                                                                            | Como mudar                         |
| --------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------- |
| Histórico de conversa       | **Para sempre** por padrão; opções: 1, 7, 30 ou 90 dias                                 | Privacidade → Retenção de conversa |
| Memória de curto prazo      | 168 horas (padrão)                                                                      | Configurações de memória           |
| Memória de longo prazo      | 365 dias (padrão)                                                                       | Configurações de memória           |
| Logs                        | Até 10 MiB por arquivo (`mika-human.log` e `mika-ai.ndjson`), sem histórico rotacionado | "Apagar meus dados"                |
| Demais dados locais         | Até você apagar                                                                         | Seção 8                            |
| Dados enviados a provedores | Conforme a política do provedor                                                         | Veja o provedor                    |

## 8. Seus direitos (LGPD e GDPR)

Você tem direito a confirmação do tratamento, acesso, correção, anonimização/bloqueio/eliminação,
portabilidade, informação sobre compartilhamento, revogação do consentimento e oposição (LGPD, art.
18; GDPR, arts. 15 a 22). Como os dados ficam no seu computador, você os exerce diretamente:

- **Acesso e portabilidade:** peça ao Mika para exportar os seus dados (ferramenta `export_data`):
  gera um `.zip` na pasta `exports` com perfil, lembretes, memória episódica, catálogo de pessoas,
  memória social, sessões de jogo, metas, **as mensagens do histórico de conversa** e os relatórios
  salvos, mais um `manifest.json`. Os contadores de tokens e os hábitos de uso **não** entram nessa
  exportação; os arquivos de dados da seção 2 podem ser copiados diretamente. O `.zip` não é
  protegido por senha.
- **Eliminação:** peça ao Mika para apagar os dados (ferramenta `clear_history`, sempre com a sua
  aprovação): apaga histórico, memória, perfil, lembretes, metas, cache de voz, catálogo de pessoas,
  memória social, sessões de jogo, capturas, segredos salvos, logs, exportações anteriores, **hábitos
  de uso, cópias pré-atualização (`backups/pre-*`) e relatórios de resposta**. **Não cobre ainda:**
  a memória da sessão atual, a telemetria local, o estado de humor, os caches de análise de tela e de
  OCR e a sessão do navegador embutido (a própria ferramenta lista, ao terminar, o que não tocou).
  Os contadores de tokens têm o botão próprio de zerar nas configurações. Para apagar tudo, feche o Mika e apague a pasta de
  dados (seção 2); a desinstalação pode não removê-la.
- **Revogar consentimentos:** desligue o recurso em Configurações (Visão, Captura de tela, Área de
  transferência, MCP, Navegador, provedores remotos) ou ligue o modo offline.
- **Dados enviados a provedores:** peça a eliminação diretamente ao provedor.
- **Reclamação:** à Autoridade Nacional de Proteção de Dados (ANPD) ou à autoridade do seu país.

Para qualquer pedido, escreva para **[E-MAIL DE SUPORTE]**. Não temos cópia das suas conversas; só
conseguimos atender o que está em nosso poder (por exemplo, registros de venda).

## 9. Bases legais

Execução do contrato de licença (fornecer o aplicativo) e **consentimento** para cada recurso
opcional, que você pode revogar a qualquer momento. `[REVISAR BASES LEGAIS COM O JURÍDICO]`

## 10. Transferência internacional

Quando você usa um provedor remoto, o dado vai para os servidores dele, que podem estar fora do seu
país. Isso decorre da sua escolha de provedor.

## 11. Crianças

O Mika é para **maiores de 18 anos**: o aceite dos termos exige marcar "Tenho 18 anos ou mais" (uma declaração sua; não verificamos a idade) e, sem essa marcação, o vínculo romântico da persona fica bloqueado. Não coletamos dados de menores de propósito; se você tem menos de 18 anos, não use o Mika. Se o Mika perceber que fala com um menor, é instruído a encerrar qualquer tom romântico.

## 12. Mudanças nesta política

A versão vigente acompanha o aplicativo. Mudanças que alterem o que sai do computador serão
comunicadas na atualização.

## 13. Contato

**[RAZÃO SOCIAL]** — [ENDEREÇO] — **[E-MAIL DE SUPORTE]** — DPO: **[ENCARREGADO (DPO)]**
