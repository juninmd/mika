# Mika — plano de arquitetura modular e isolamento de execução

> **REGISTRO HISTÓRICO (2026-09-07).** Plano de uma arquitetura TypeScript (bibliotecas, DI, `packages/*`, sidecar) que a migração para Tauri/Rust substituiu: caminhos, nomes e o estado descrito aqui NÃO refletem o repositório atual. A fonte de verdade da arquitetura é `AGENTS.md` §2 e `docs/tauri-rust-migration.md`.

Status: proposta de implementação; não representa uma migração já realizada ou uma homologação do app.

Data: 2026-09-07. Base da revisão: commit `47408f55` e código inspecionado nesta sessão.

Leitor: mantenedor que vai extrair, testar e migrar as capacidades da Mika. Este guia responde como transformar cada feature em uma biblioteca independente, mantendo o app responsivo e verificando sua integração real.

Checkpoint de execução: veja a [seção 17](#17-checkpoint-de-implementação-e-adaptação), com estado atual, evidências e próximo passo. A segunda revisão detalha ownership e dependências (4.1–4.2), nomenclatura inspirada no Angular (5.1), montagem concorrente (6.1), contratos de integração (7.4) e homologação com escopo verificável (10.1–10.3).

## 1. Resultado pretendido

Cada capacidade terá API pública pequena, dependências explícitas, estado por instância, dados sob sua responsabilidade e testes independentes do Electron. O Desktop continuará sendo um único produto distribuído junto, responsável por compor essas bibliotecas.

Isolamento tem quatro dimensões diferentes:

| Dimensão | Mecanismo                                                | Prova necessária                                              |
| -------- | -------------------------------------------------------- | ------------------------------------------------------------- |
| Código   | Pacotes, exports explícitos e grafo permitido de imports | Consumidor externo compila sem alcançar implementação privada |
| Estado   | Fábricas e recursos pertencentes à instância             | Duas instâncias simultâneas não interferem uma na outra       |
| Execução | Worker ou processo para trabalho pesado                  | Main e renderer continuam respondendo durante carga           |
| Falha    | Supervisão, timeout, limites e degradação                | Queda ou travamento do executor não derruba o restante do app |

Uma biblioteca pode atender às duas primeiras linhas e ainda congelar a interface. Separar pastas não move o código para outra thread.

## 2. Os processos vão travar a thread principal?

Trabalho em outro processo não executa diretamente no event loop do Main. Entretanto, o Main ainda pode bloquear ao preparar dados, serializar mensagens, processar respostas enormes, executar código síncrono ou receber mais eventos do que consegue consumir. Processos também competem por CPU, RAM, GPU e disco. Portanto, isolamento de processo reduz o acoplamento de execução; não garante responsividade por si só.

Declarar uma função `async`, usar `Promise.all` ou colocar um cálculo em `setTimeout` não transfere esse cálculo para outra thread. Código JavaScript síncrono continua ocupando a thread em que foi chamado. I/O assíncrono e computação pesada precisam de tratamentos diferentes.

A recomendação do Electron é evitar CPU prolongada, I/O bloqueante e IPC síncrono no Main. Workers servem para computação JavaScript; processos dedicados fazem sentido quando a contenção de falhas e o lifecycle independente justificam o custo. Referências: [performance do Electron](https://www.electronjs.org/docs/latest/tutorial/performance), [Worker Threads do Node](https://nodejs.org/api/worker_threads.html) e [utilityProcess](https://www.electronjs.org/docs/latest/api/utility-process).

### 2.1 Distribuição de execução proposta

| Local                                 | Trabalho permitido                                                          | Trabalho a manter fora                                                                         |
| ------------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Main Electron                         | Janelas, permissões, IPC validado, composição, supervisão e decisões curtas | ONNX, OCR pesado, embeddings locais, compressão, grandes varreduras e transformações síncronas |
| Renderer                              | React, interação, Live2D/VRM, captura e playback pelas APIs web             | Node, banco direto, processamento pesado em handlers de UI                                     |
| AudioWorklet                          | Processamento de áudio com trabalho curto e previsível                      | Rede, persistência, LLM e alocações volumosas                                                  |
| Utility de inferência existente       | Modelos locais e seus recursos                                              | Acesso irrestrito à UI e decisões de autorização                                               |
| Worker de computação, quando medido   | Transformações CPU-bound de dados serializáveis                             | APIs Electron dependentes do Main e acesso ao DOM                                              |
| Executor de dados, se o perfil exigir | Indexação, exportação/importação e trabalho de banco que bloqueie           | Propriedade das regras de negócio de todas as features                                         |
| Processos de integrações externas     | Trabalho de subprocessos e servidores MCP                                   | Aprovação de ações privilegiadas fora do broker do host                                        |

Não criar um processo por feature. Muitas features são leves e passam a maior parte do tempo esperando I/O. Começar com as fronteiras já necessárias e introduzir executores adicionais a partir do perfil de CPU, custo de serialização e contenção de falhas.

STT e TTS podem continuar inicialmente no utility existente. As filas por categoria não garantem execução simultânea se o motor bloqueia o event loop desse utility. Se STT atrasar TTS ou vice-versa sob carga medida, comparar dois utilities independentes com um único executor de concorrência limitada, incluindo RAM duplicada e número de threads nativas.

Workers não constituem uma fronteira de segurança para código não confiável. Processos também precisam de políticas de acesso; autorizações continuam no host.

### 2.2 Evidência do estado atual

- ~~`apps/desktop/src/main/services/inference/client.ts`~~ já usa `utilityProcess.fork`, timeout de prontidão e limitação de reinícios. O envio de trabalho rejeita quando o host não está disponível, protegendo o Main.
- Nesse mesmo arquivo há mensagens antigas dizendo que a inferência voltará ao Main. Elas contradizem a rejeição observada em `run`; corrigir essa comunicação na etapa de execução, sem introduzir o fallback perigoso.
- ~~`apps/desktop/src/main/services/inference/host.ts`~~ documenta que trabalho local já iniciado não pode ser preemptado pelo cancelamento. Descartar a resposta não interrompe o consumo de CPU do motor.
- ~~`apps/desktop/src/main/services/inference/job-queue.ts`~~ encadeia Promises e não declara capacidade máxima. Serialização de execução não equivale a limitação de backlog.

Essas observações vêm do código, não de um benchmark de responsividade executado nesta revisão.

## 3. Problemas da fronteira atual

| Evidência                                                                                                          | Mudança proposta                                                                   |
| ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| ~~`apps/desktop/src/main/services/brain/ports.ts`~~: `active` global e injeção parcial com fallback para container | Dependências completas por instância, sem fallback implícito                       |
| Scheduler tinha store e prontidão globais (um módulo global, removido na etapa 1)                                  | Estado dentro da instância do scheduler                                            |
| `packages/sdk/package.json`: exports curingas e dependências amplas                                                | Superfícies explícitas e implementação de domínio nas bibliotecas donas            |
| ~~`apps/desktop/src/main/services/bootstrap/chat.ts`~~: serviço de conversa dentro de brain e histórico em chat    | Fronteira funcional de conversation                                                |
| `scripts/checks/boundaries.mts`: inspeção de imports relativos com `from`                                          | Resolver grafo, incluindo aliases, imports dinâmicos e dependências transitivas    |
| ~~`packages/sdk/src/services/event-bus.ts`~~: publicação aguarda listeners sem deadline local                      | Separar comandos de observação e definir política de entrega                       |
| `scripts/homologate.mts`: ausência de suíte e falha headless não bastam para exit code não zero                    | Critérios explícitos por tipo de módulo, com reprovação para biblioteca incompleta |

O comando existente `pnpm -s homologate scheduler` passou nesta revisão. A suíte inspecionada contém sete casos de cálculo de horário. Isso não comprova persistência, ativação e disparo integrados.

## 4. Bibliotecas e ownership

Os nomes abaixo são destinos propostos, não pacotes já extraídos.

| Biblioteca      | Responsabilidade e dados                                       | Integrações por contrato                                  |
| --------------- | -------------------------------------------------------------- | --------------------------------------------------------- |
| conversation    | Turnos, histórico, stream, cancelamento e coordenação de tools | LLM, memória, contribuições de contexto e broker de tools |
| memory          | Memória episódica/social, recuperação e consolidação           | Embedding, repositório e extração                         |
| llm             | Provedores, ZDR, failover, deadlines e normalização            | Clientes remotos ou Ollama e política de rede             |
| voice           | Estados de escuta/fala e coordenação STT/TTS                   | Captura, playback e inferência                            |
| vision          | Observação, OCR, interpretação e validade do contexto visual   | Captura por SO, processamento e LLM                       |
| browser         | Sessões e navegação                                            | Executor de automação e autorização                       |
| scheduler       | Agendamento, recorrência e estado de entrega                   | Relógio, repositório, notificação e fala                  |
| mood            | Humor e afinidade; único escritor desses estados               | Comandos de interação e snapshots                         |
| character       | Personalidade e apresentação do personagem                     | LLM, configuração e capacidades do avatar                 |
| person-catalog  | Pessoas, edição, identidade e consentimento do catálogo        | Repositório e portas de memória social                    |
| gamer-companion | Sessões de jogo, contexto e sinais de proatividade             | Detecção de jogo, vision e conversation                   |
| user            | Perfil e preferências do usuário                               | Persistência e contribuições de contexto                  |
| desktop         | Ações no sistema operacional                                   | Broker privilegiado e adaptadores por SO                  |
| mcp             | Conexões, inventário e lifecycle dos servidores                | Processos/transporte e broker de aprovação                |
| privacy         | Preferências, redação e pedidos de exportação/exclusão         | Política central do host e APIs dos donos dos dados       |
| telemetry       | Diagnóstico sanitizado e métricas                              | Sink e política de privacidade                            |
| update          | Verificação e aplicação de atualização                         | Adaptador de atualização por plataforma                   |

O Desktop compõe, registra IPC, mantém o roster explícito e supervisiona recursos. Ele não se torna dono das regras de todas as bibliotecas. Segurança é obrigatória no host mesmo quando a UI de privacy não foi ativada.

Retirar do brain as responsabilidades com destino claro. Composição de contexto e proatividade começam como módulos internos com APIs pequenas; extrair um pacote adicional somente quando houver uma fronteira independente demonstrável.

### 4.1 Responsabilidades que não podem ter dois donos

O catálogo de pacotes acima não basta para decidir quem altera um dado ou resolve um conflito. Esta matriz fecha as fronteiras que ainda estavam ambíguas na primeira versão da proposta:

| Conceito                     | Dono da decisão                    | Limite dos demais participantes                                                                                                        |
| ---------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Humor e afinidade            | mood                               | Character define personalidade e preferências; conversation consome snapshot; avatar representa a expressão sem escrever afinidade     |
| Identidade de pessoa         | person-catalog                     | Memory referencia PersonId e guarda fatos com origem/confiança; não mantém uma segunda autoridade sobre nomes e consentimento          |
| Perfil do dono do app        | user                               | Usar vínculo explícito com PersonId quando existir; não duplicar o perfil como outra pessoa independente                               |
| Histórico de conversa        | conversation                       | Memory recebe fatos extraídos e referências aos turnos; não se torna um segundo histórico editável                                     |
| Intervenção proativa         | Arbitragem interna de conversation | Gamer-companion, vision e scheduler oferecem candidatos com origem, validade e prioridade; não iniciam turnos concorrentes diretamente |
| Sessão de fala e interrupção | voice                              | Conversation autoriza a intervenção; voice controla fila de fala, cancelamento e confirmação; renderer executa e confirma playback     |
| Notificação de lembrete      | scheduler                          | A notificação local continua independente da disponibilidade de fala; falha da fala não apaga o resultado da notificação               |
| Política de autorização/rede | Host de segurança                  | Privacy fornece preferências; llm, browser, mcp e desktop aplicam o acesso recebido sem poder desligar a política                      |
| Configuração de negócio      | Biblioteca dona da chave           | UI usa a API da biblioteca; serviço de configuração persiste e distribui, sem decidir comportamento                                    |

Evidência de necessidade: ~~`apps/desktop/src/features/mood/index.ts`~~ lê/escreve afinidade sob character; ~~`apps/desktop/src/main/services/brain/services/brain.service.ts`~~ também mantém estado emocional; ~~`apps/desktop/src/features/person-catalog/ports.ts`~~ alcança memória social por BrainService. A migração deve mapear significado, escala e conversão dos dados antes de unificar seus escritores, sem simplesmente renomear chaves.

O árbitro de conversation decide entre turno solicitado pelo usuário, lembrete e observação espontânea, com prioridade, prazo e cooldown declarados. Voice decide a ocupação do dispositivo e confirma started/completed/interrupted. Os dois contratos são distintos. Testar lembrete vencendo durante conversa, dois candidatos no mesmo instante, mute, interrupção e término tardio de áudio cancelado.

Exportação/exclusão de dados é coordenada por privacy chamando APIs dos donos. A operação tem ID, progresso e resultado por participante; falha parcial não responde sucesso integral. Incluir fatos derivados, embeddings, histórico e arquivos temporários no inventário, com regras explícitas de retenção. Não conceder a privacy acesso SQL irrestrito a todos os módulos para facilitar a integração.

### 4.2 Dependências permitidas e tamanho do módulo

Cada pacote terá uma lista fechada de entrypoints consumíveis. A direção proposta é:

| Origem                | Pode depender de                                  | Não pode depender de                                                                       |
| --------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Contratos públicos    | DTOs/schemas de conceitos com dono, sem runtime   | Implementações, React, Electron, driver SQL ou cliente LLM                                 |
| Regras e casos de uso | Próprias portas, contratos e funções puras        | Container, window, process.env, rede/disco globais ou serviço concreto de outra biblioteca |
| Adaptador             | Contrato implementado e driver necessário         | Internos do consumidor ou política de produto alheia                                       |
| UI da feature         | Próprio contrato, cliente de UI injetado e SDK UI | Runtime do Main, banco ou serviço de outra feature                                         |
| Composição do Desktop | Exports públicos de fábricas/adaptadores          | Caminhos privados de bibliotecas                                                           |

O host pode mapear uma porta do consumidor para o contrato do provedor. Se os contratos se referenciam mutuamente, escolher um dono do conceito ou reconsiderar a divisão; esconder o ciclo em eventos/registro global não o resolve.

Portas encapsulam capacidades reais, como consultar lembretes ou sintetizar áudio. Não criar uma interface por função apenas para permitir mocks. Submódulos que compartilham invariantes e transações podem permanecer juntos; não tratar todo arquivo como uma biblioteca independente.

Configuração, relógio, gerador de IDs e aleatoriedade entram por instância quando influenciam o resultado. Snapshot público não expõe objeto mutável interno. Eventos de dados do domínio usam payload validado e ownership definido; o consumidor não modifica o estado do produtor por referência. Buffers de áudio/imagem seguem a política de transporte da seção 8, sem cópias profundas indiscriminadas.

## 5. Layout e superfície pública

Árvore ilustrativa futura de uma biblioteca:

```text
packages/scheduler/
  package.json
  tsconfig.json
  vitest.config.mts
  src/
    index.ts
    scheduler.contracts.ts
    scheduler.schema.ts
    scheduler.module.ts
    scheduler.feature.ts
    ports.ts
    services/
      scheduler.runtime.ts
      scheduler.service.ts
    tools/
      reminder.tool.ts
    handlers/
      schedule-reminder.handler.ts
    adapters/
      sqlite-reminder.repository.ts
    renderer/
      scheduler-settings.component.tsx
    prompt-loader.ts
    prompts/
    __tests__/
      scheduler.contract.spec.ts
```

Criar somente as pastas necessárias. Manter nomes compostos com ponto e `index` reservado ao barril. A convenção de nomes de arquivos proposta está na seção 5.1; símbolos de componentes React continuam em PascalCase. Preservar a relação do prompt-loader com prompts; testar os assets no bundle empacotado.

Exports públicos propostos: raiz para fábrica e tipos essenciais; contracts para schemas/DTOs sem dependências de ambiente; renderer para a UI. Adaptadores que o host realmente precisa construir recebem exports nominados, sem liberar toda a árvore. Dublês ficam em suporte de testes e não no bundle de produção.

Cada pacote terá dependências declaradas, typecheck e testes próprios. Seus consumidores não poderão importar arquivos de apps nem ultrapassar os exports. Um consumidor de teste fora da pasta do pacote deverá carregar o artefato construído, incluindo tipos e assets, para evitar sucesso baseado em aliases privados do monorepo.

O entrypoint de renderer não pode alcançar Node, Electron, banco ou o entrypoint de runtime, nem transitivamente. Um mesmo pacote pode conter UI e runtime, mas seus grafos precisam ser separados.

O SDK atual será reduzido por uso: retirar implementações após migrar seus consumidores, mantendo fachadas temporárias explícitas. Não criar um novo pacote comum que reúna toda a aplicação novamente.

### 5.1 Convenção inspirada no Angular clássico

Escolha para esta proposta: arquivos de código em `assunto.papel.ts`, com palavras do assunto separadas por hífen. Para React, usar `.tsx`. Esse é o estilo clássico documentado no [Angular v18](https://v18.angular.dev/style-guide/#style-02-02). O [guia atual do Angular](https://angular.dev/style-guide) usa exemplos sem sufixo de papel, como user-profile.ts; a proposta adota deliberadamente a variante clássica preferida pelo mantenedor, sem afirmar que ela é uma exigência atual do framework.

| Papel                         | Nome proposto                    | Responsabilidade                                                                                     |
| ----------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Manifesto estático            | scheduler.module.ts              | Identidade e fronteira declarada; não instancia serviços e não é NgModule                            |
| Integração de produto         | scheduler.feature.ts             | Fábrica da contribuição de tools/settings ligada à instância; não usa singleton nem redefine schemas |
| Montagem e lifecycle          | scheduler.runtime.ts             | Exporta createScheduler e coordena aquisição/liberação de recursos                                   |
| Regras/coordenador do domínio | scheduler.service.ts             | Executa o comportamento; não resolve container                                                       |
| Contratos públicos            | scheduler.contracts.ts           | DTOs, resultados e API pública estável                                                               |
| Configuração                  | scheduler-config.schema.ts       | Schema da config; distinguir de schema de entrada da tool                                            |
| Porta de dependência          | ports.ts                         | Interfaces exigidas; nome reservado do módulo, sem registro global                                   |
| Tool declarada                | reminder.tool.ts                 | Schema e declaração da tool ligada à instância                                                       |
| Implementação de operação     | schedule-reminder.handler.ts     | Caso de uso com decisão/validação; dispensar se apenas repassar uma chamada                          |
| Adaptador de persistência     | sqlite-reminder.repository.ts    | Implementa a porta de repositório com o driver SQLite                                                |
| Adaptador de transporte       | scheduler-ipc.adapter.ts         | Mapeia contrato e erros; código Electron no host                                                     |
| Componente React              | scheduler-settings.component.tsx | Exporta SchedulerSettings; JSX e apresentação                                                        |
| Hook React                    | use-reminders.ts                 | Exporta useReminders; estado de apresentação e chamada ao cliente injetado                           |
| Teste de regra                | scheduler.service.spec.ts        | Testa regras do serviço                                                                              |
| Teste da biblioteca           | scheduler.contract.spec.ts       | Exercita API pública, lifecycle e duas instâncias                                                    |

O nome de arquivo fica em kebab-case; classes, tipos e componentes seguem PascalCase, funções seguem camelCase e hooks começam com use. Não usar nomes de arquivo PascalCase no novo padrão, inclusive para React. Isso é uma proposta de uniformização da convenção permissiva atual; coordenar a atualização do AGENTS na etapa 2, sem renomear a árvore existente nesta revisão.

Não criar variantes concorrentes como scheduler-service.ts, scheduler.svc.ts ou SchedulerService.ts. Evitar arquivos vagos como utils.ts, common.ts, manager.ts e vários settings.tsx indistinguíveis na busca. Uma função pura pode ter nome descritivo sem sufixo técnico, como next-local-time.ts; não transformá-la em classe Service apenas pela nomenclatura.

index.ts contém reexports explícitos. O host importa a fábrica pela API pública; o barril não abre recursos nem monta o app. O manifesto não importa runtime; a contribuição de feature recebe a instância pronta, em vez de criar tools globais no topo do arquivo. Identidade e referências a schemas vêm de definições canônicas, sem cópias divergentes entre module, feature e registry.

Manter inicialmente as pastas locais exigidas pelo AGENTS. A organização principal já é por domínio: não criar uma pasta services global reunindo todas as features. Quando uma biblioteca crescer, propor subáreas de negócio coesas, sem repetir camadas vazias para cada operação. Esta adaptação preserva o layout Mika; não pretende reproduzir toda a organização de diretórios do Angular.

Usar `.spec.ts` nos novos testes unitários/contratuais e preservar `.test.ts` existente até migração do módulo. O Vitest atual reconhece ambos. Specs com JSX exigem configuração explícita, pois o include atual contempla apenas `.ts`; UI continua dependendo do Playwright real. Suítes contratuais ficam em `__tests__` conforme a convenção Mika, em vez de impor uma reorganização geral de testes junto ao código.

Renomes devem atualizar imports, exports, rosters, manifestos, glob de testes, docs e inventário de assets no mesmo passo. Validar em filesystem com distinção de maiúsculas/minúsculas antes de aceitar suporte Linux. Não mover prompt-loader nesta padronização. Gate futuro de nomenclatura deve respeitar esses nomes reservados e impedir novas variantes nos pacotes já migrados.

## 6. Montagem, dependências e lifecycle

API conceitual, não exemplo executável já implementado:

```text
createScheduler({ repository, clock, generateSpeech,
                  publishNotification, requestSpeech, config })
  -> { start, schedule, cancel, list, health, dispose }
```

1. Importar declara código; não abre banco, inicia timer, carrega modelo ou conecta rede.
2. A fábrica valida as dependências e cria estado local. Não consulta container global.
3. `start` adquire recursos e só publica prontidão depois de completar a montagem.
4. Se a ativação falhar, libera o que já adquiriu e publica estado de falha.
5. `dispose` deixa de aceitar trabalho, cancela pendências, remove listeners, encerra recursos próprios e é idempotente.
6. Recursos emprestados pelo host não são encerrados pela biblioteca. O contrato identifica quem possui cada conexão/executor.

Estados propostos: created, starting, ready, degraded, stopping, stopped e failed. Feature desabilitada é estado deliberado, distinguível de falha. `health` inclui razão e capacidades disponíveis, sem credenciais ou dados pessoais.

Dependência obrigatória ausente falha de forma explícita. Dependência opcional ausente tem degradação definida. O scheduler pode manter notificação local quando a fala estiver indisponível, sem afirmar que falou.

O container permanece na composição do host. `ports.ts` dentro da biblioteca passa a declarar interfaces, sem resolver tokens. A adaptação container → fábrica fica no host. Isso altera a convenção atual e requer atualização coordenada das instruções e dos gates na etapa correspondente.

### 6.1 Boot, concorrência de lifecycle e desligamento de feature

O roster continua explícito. O host valida dependências obrigatórias e monta um grafo acíclico antes de ativar; dependência ausente produz diagnóstico, nunca auto-instanciação. Boot mínimo inclui segurança, janelas e IPC essencial; bibliotecas de negócio entram por necessidade com concorrência limitada. A ordem de boot não define o domínio do pacote.

Cada comando espera somente suas capacidades obrigatórias por prazo limitado. Não esperar todas as features tardias para enviar uma mensagem. Hoje ~~`apps/desktop/src/main/services/chat/ipc.ts`~~ espera waitForDeferred; adaptar essa espera na migração, preservando dependências realmente necessárias. Testar falha de uma feature opcional enquanto chat e janela continuam disponíveis.

Semântica de lifecycle proposta:

- Duas chamadas concorrentes de start compartilham a mesma tentativa; não criam recursos duplicados.
- Dispose durante starting cancela a tentativa, executa a limpeza parcial e impede publicação tardia de ready.
- Stopped é terminal para a instância; reativar cria outra instância e outra geração, sem reaproveitar recursos encerrados.
- Registro de tools, listeners e UI retorna um handle de liberação pertencente à instância. Desabilitar revoga esses registros e trata chamadas em voo conforme o contrato.
- Resultado de geração encerrada não aplica estado nem agenda nova fala.
- Config inválida é rejeitada antes de persistir/aplicar; mudanças válidas têm revisão e aplicação consistente. Se não puderem ser aplicadas ao vivo, o contrato informa a necessidade de reinício da capacidade.

O host encerra consumidores antes de provedores, com prazo por recurso. Nenhum módulo pode limpar o barramento inteiro, descarregar registros alheios ou fechar conexão emprestada. Essas propriedades exigem teste de integração além do teste A/B isolado.

## 7. Comunicação

Proposta de mudança à regra atual de tudo passar por EventBus: comandos e consultas usam portas tipadas; eventos comunicam fatos. Até a adoção coordenada nas specs, as regras atuais continuam valendo.

O host conecta a porta consumida à API pública do provedor. Bibliotecas não importam implementações irmãs. Portas específicas pertencem ao consumidor; DTOs compartilhados têm um dono explícito no contrato público. Evitar dependências circulares de tipos e schemas.

### 7.1 Comando em memória

Definir entrada, resultado, erro, cancelamento e semântica de repetição. Aprovação negada, indisponibilidade e timeout são resultados distinguíveis. Uma tool converte o resultado para o contrato existente de sucesso/erro e não lança para o loop de ferramentas.

### 7.2 Comando entre processos

Envelope proposto:

| Campo           | Função                                                               |
| --------------- | -------------------------------------------------------------------- |
| protocolVersion | Rejeitar versões incompatíveis                                       |
| requestId       | Correlacionar uma chamada única                                      |
| generation      | Distinguir uma instância de executor após reinício                   |
| operation       | Operação fechada e validada                                          |
| deadlineAtMs    | Prazo fim a fim; expiração conferida ao receber e ao retirar da fila |
| payload         | DTO validado e limitado por bytes                                    |
| idempotencyKey  | Obrigatória para efeitos persistentes que aceitem retry              |

Resposta inclui identidade da chamada e resultado ou erro tipado. Não enviar funções, container ou AbortSignal pelo transporte. Traduzir o signal local em mensagem de cancelamento por requestId. Medir durações com relógio monotônico local; mudança do relógio civil não deve prolongar uma chamada já admitida.

Uma resposta atrasada ou de generation anterior não altera o estado atual. Queda rejeita pendências exatamente uma vez. Retry só ocorre se o contrato da operação permitir; timeout não prova que uma escrita falhou.

### 7.3 Eventos

Cada evento declara dono, payload, consumidores, ordenação e política de falha. Observação não é confirmação de execução. Publicar `SpeechRequested` não significa que o áudio tocou; o estado depende da confirmação do dono do playback.

Para trabalho confiável, usar comando/ack explícito ou registro persistente com deduplicação. Para observação efêmera, permitir coalescer atualizações e descartar contexto vencido. Não criar um barramento genérico com fila infinita.

### 7.4 Contratos do domínio e contrato central do Desktop

A biblioteca é dona do schema e dos erros de sua operação. O contrato central de IPC continua sendo o catálogo explícito e tipado das rotas do Desktop, compondo/reexportando esses contratos sem duplicar DTOs manualmente. O catálogo central de eventos faz o mesmo para valores e payloads públicos, sem importar implementações.

No renderer, componentes recebem um cliente tipado restrito à feature. O host de UI implementa esse cliente usando window.electronAPI. Testar o componente com cliente local não substitui o teste Playwright que passa pelo preload e Main reais.

O handler do Main valida payload e tamanho, verifica a superfície/janela com assertOwnerSurface quando sensível, aplica autorização e só então chama a API pública. O domínio repete validações de negócio necessárias a qualquer entrada; não confia na UI. Identidade do chamador vem do contexto confiável do host, não de um campo fornecido pelo renderer ou LLM.

Qualquer erro serializado pertence a um catálogo pequeno e discriminado, com mensagens sanitizadas. Não devolver stack, token, SQL ou caminho pessoal para a UI. Um adaptador deve traduzir protocolo, segurança, ambiente ou dados; remover adaptadores que apenas criem uma passagem sem responsabilidade.

## 8. Limites e supervisão dos executores

Antes de mover trabalho para um executor:

1. Medir custo no chamador, no executor e na serialização.
2. Classificar se a tarefa é interativa, observação substituível ou manutenção.
3. Definir capacidade de fila em itens e bytes, concorrência e tamanho máximo de payload.
4. Definir resultado quando lotado: busy, substituição do item antigo ou adiamento explícito.
5. Conferir prazo antes de iniciar e cancelar pendências expiradas.
6. Agregar chunks e telemetria para não inundar Main/renderer.

Políticas iniciais para POC, ajustáveis por medição: vision mantém no máximo a observação pendente mais recente; indexação roda com concorrência 1 e lotes limitados; tarefas interativas têm prioridade com justiça para evitar inanição; filas de fala têm teto de duração de áudio, além de quantidade de mensagens. Fixar os valores em configuração de executor antes de aceitar sua migração, documentando hardware e carga usados.

Executores reutilizáveis substituem a criação de worker a cada chamada. Limitar também threads internas dos motores nativos quando suportado. A concorrência JavaScript não representa o total de threads de CPU/GPU.

Não pressupor zero-copy de áudio/imagem entre todos os processos. Verificar suporte real do transporte. Preferir binário, limitar buffers e evitar base64 volumoso; usar transferência de ownership somente onde suportada e testada. SharedArrayBuffer não é pré-requisito desta migração.

Supervisor do host possui o filho e seus timers. Em crash, marca capacidade indisponível, rejeita chamadas, reinicia com backoff limitado e circuit breaker. Em hang, o watchdog do Main detecta falta de progresso mesmo se o event loop do filho não responder. Encerrar somente o filho pertencente à aplicação e registrar quais tarefas foram interrompidas.

No utility compartilhado, encerrar para interromper uma tarefa também interrompe as demais. Essa consequência exige teste e comunicação de degradação. Não migrar inferência pesada de volta ao Main como fallback. Oferecer recuperação ou outra capacidade compatível com a política do usuário, preservando offline/ZDR e consentimento.

## 9. Persistência e consistência

Cada dado tem um dono de escrita: afinidade em mood, lembretes em scheduler, pessoas no catálogo e memória em memory. Outras bibliotecas usam comandos públicos.

Uma infraestrutura pode administrar conexões e locks. Cada biblioteca possui schema, migrações e operações de seu repositório. Não exigir um arquivo SQLite por biblioteca; definir explicitamente transações que cruzariam donos e redesenhar o fluxo antes de separá-las.

Para lembretes, persistir identidade, prazo, recorrência, estado de notificação e estado/tentativas de fala separadamente. Reprocessamento após crash precisa deduplicar a notificação pelo identificador da ocorrência. Fala é efeito externo: documentar a janela entre tocar áudio e persistir confirmação; não prometer exactly-once sem mecanismo que a sustente.

Migração usa expandir → transferir ownership → remover compatibilidade. Nunca dois escritores ativos para o mesmo dado. Fazer backup e ensaio de restauração antes de mudança destrutiva. Rollback de código só é válido enquanto o schema continuar legível pela versão anterior.

## 10. Estratégia de testes

| Camada            | Dependências                                        | Aceite                                                     |
| ----------------- | --------------------------------------------------- | ---------------------------------------------------------- |
| Unidade           | Relógio e entradas controlados                      | Regras, erros e transições determinísticos                 |
| Biblioteca        | Implementação real; dublês apenas de borda          | Fluxos completos da API, duas instâncias e teardown        |
| Adaptador         | SQLite temporário real ou transporte correspondente | Mesmo contrato respeitado pelo adaptador real e pelo dublê |
| Integração        | Bibliotecas reais conectadas                        | Fluxos entre módulos, falhas e recuperação                 |
| E2E               | Electron real                                       | Boot, IPC, UI e ações observáveis                          |
| Carga/recuperação | App e executores reais                              | Responsividade, limites, crashes e recursos liberados      |

Vitest terá projetos por biblioteca e ambiente Node para lógica. Playwright continua sendo a prova de UI, com screenshots abertos após mudanças de tela. Contratos de integração headless rodam na camada Vitest quando a pergunta não depende de Electron/DOM. Não substituir lógica de produção por mocks.

Comando futuro, disponível somente após criar o pacote e seu script test:

```powershell
pnpm --filter @mika/scheduler -s test
```

Teste obrigatório de independência: criar A e B no mesmo contexto de módulos, com bancos/configurações distintos, executar ambos e encerrar A; B continua funcionando. Repetir em ordem inversa. Limpar módulos entre os casos não pode ser o mecanismo que produz o isolamento.

Testes de contrato não bastam para drivers reais. Validar pelo menos uma integração real de cada provedor suportado antes de afirmar funcionamento; falha por credencial ausente é bloqueio declarado, não aprovação. Nunca imprimir credenciais ou enviar dados pessoais de teste.

### 10.1 O que significa homologar com qualidade

Não é possível garantir 100% de precisão e ausência de defeitos de um módulo não trivial. Testes cobrem casos e modelos de comportamento definidos; não todas as entradas, combinações, versões de drivers, condições de hardware ou respostas futuras de provedores. Verificação formal pode provar propriedades de um modelo com hipóteses explícitas, não automaticamente do Electron inteiro.

O compromisso verificável é: **todos os critérios obrigatórios do escopo declarado têm evidência válida, e nenhuma lacuna é registrada como aprovação**. Isso pode ser 100% dos critérios acordados, mas não 100% de qualidade universal.

Cada biblioteca terá uma matriz versionada: requisito → cenário → oráculo → teste → artefato → ambiente. O oráculo é o que decide se o resultado está correto; não basta comparar a função com uma cópia da própria implementação. Na revisão da matriz, procurar requisitos ausentes além de conferir os testes existentes.

Exemplo de matriz mínima para scheduler:

| Requisito           | Cenário e oráculo                                                                       | Prova                                                                  |
| ------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Independência       | A e B usam stores/config distintos; encerrar A preserva B                               | Contrato por instância no mesmo contexto de módulos                    |
| Agendamento correto | Prazo futuro, fuso e transição de horário; resultado esperado independente do algoritmo | Casos de borda e propriedades temporais                                |
| Persistência        | Agendar, fechar, reabrir banco e consultar a mesma ocorrência                           | Repositório real temporário                                            |
| Entrega sob falha   | Falha de LLM preserva notificação e respeita limite de tentativas                       | Biblioteca real com falha na borda                                     |
| Concorrência        | Dois ticks sobre a mesma ocorrência não criam duas confirmações de notificação          | Contrato de claim/deduplicação do repositório real                     |
| Encerramento        | Dispose durante start/tick; nenhuma execução tardia                                     | Lifecycle com relógio controlado e recursos observáveis                |
| Integração          | Usuário agenda pela UI, reinicia o app e observa o disparo                              | Playwright com banco e runtime reais; prova da fala em integração real |

Claims de ocorrências e deduplicação são transacionais no repositório dono. Duas instâncias de teste têm namespaces independentes; isso não autoriza dois donos do mesmo banco de produção. Se múltiplos executores acessarem o mesmo dado por projeto, documentar leases, expiração e recuperação de claim, com testes adicionais.

### 10.2 Qualidade das asserções e comportamento de LLM

Coverage informa execução de linhas/ramos, não suficiência das asserções. Usar cobertura de ramos para procurar áreas esquecidas e mutation testing seletivo para avaliar se os testes detectam mudanças de decisão em aprovação, retry, fila, cancelamento e recorrência. Um mutante sobrevivente merece análise; mutante equivalente ou timeout não se torna sucesso sem justificativa. Referências: [coverage do Vitest](https://vitest.dev/guide/coverage) e [métricas de mutação do Stryker](https://stryker-mutator.io/docs/mutation-testing-elements/mutant-states-and-metrics/).

Planejar mutation testing como gate da fase de homologação, com escopo e orçamento explícitos, integrado ao fluxo obrigatório quando for implementado. Não instalar a ferramenta nesta revisão nem executar mutação indiscriminada em todo o app. O limiar e exclusões precisam de baseline revisada; nenhum número de coverage/mutation deve ser traduzido como probabilidade de o módulo não ter bugs.

Para propriedades com muitas combinações, gerar entradas/sequências e guardar a seed do caso que falhou. Exemplos: tentativas nunca excedem o limite; cancelar antes de iniciar impede o efeito; sequência start/dispose não deixa listener órfão. Combinar propriedades com exemplos de negócio independentes do código.

LLM tem duas avaliações diferentes:

- Contrato determinístico: serialização não vazia, schema da tool, ZDR, timeout, aprovação, limites de custo e resposta a falhas. Dublê de provedor é apropriado para esses testes de borda.
- Qualidade empírica: respostas úteis, sem repetição indevida, seleção correta de tools e coerência com contexto. Usar corpus versionado, rubrica explícita, casos negativos, amostra por provedor/modelo e revisão humana dos resultados relevantes. Um LLM juiz não é oráculo infalível.

Registrar versão do provedor/modelo quando disponível, parâmetros, prompts, corpus, número de execuções, taxa de sucesso e incerteza da estimativa. Não comparar a fala com uma única string esperada como prova de qualidade. Não afirmar que temperatura zero garante repetibilidade entre versões/infraestruturas. Atualização de modelo ou prompt reabre o aceite correspondente.

### 10.3 Relatório de homologação vinculado ao artefato

O comando homologate deve evoluir para produzir resultado estruturado e resumo legível, com os seguintes campos. Este formato ainda é um entregável futuro; o script atual não o produz.

| Campo                   | Conteúdo exigido                                                                    |
| ----------------------- | ----------------------------------------------------------------------------------- |
| Identidade              | Módulo, versão e perfil de homologação                                              |
| Artefato                | SHA, alterações locais ou digest da árvore testada, digest do bundle e assets       |
| Dependências            | Lockfile, versões dos adaptadores, runtime, schemas de banco e contratos consumidos |
| Ambiente                | SO, hardware relevante, config sem segredos, modelo e permissões exercitadas        |
| Cobertura de requisitos | Critérios obrigatórios, cenário/oráculo e resultado individual                      |
| Evidência               | Comandos, exit codes, logs sanitizados, traces, métricas e screenshots aplicáveis   |
| Estado                  | Aprovado no escopo, reprovado, bloqueado por ambiente ou não executado              |
| Limites                 | Casos/plataformas não cobertos, exclusões justificadas e próximos passos            |

Perfis separados: biblioteca headless; adaptadores; integração Desktop; plataforma/carga. Aprovar biblioteca não aprova automaticamente seus adaptadores ou UI. Módulo sem UI não precisa inventar uma tela de teste, mas um adaptador Electron continua precisando de prova no Electron.

Um perfil só retorna aprovação se todos os critérios obrigatórios forem executados e passarem. Ausência de teste, dependência de ambiente indisponível ou skip obrigatório bloqueia a aprovação; no pipeline de homologação, isso retorna código não zero. Retry que esconde flakiness não muda falha para aprovação sem registrar e resolver a causa. Não usar simples presença de arquivo de teste como critério de fechamento.

Hash somente da API pública detecta mudanças de superfície; não detecta alteração de algoritmo interno, prompt, asset ou dependência. O certificado vale para o artefato testado. Alteração de implementação invalida o aceite do módulo; contrato compartilhado invalida consumidores afetados; mudança de host, preload ou transporte exige repetir integração correspondente; mudança de driver/modelo/SO reabre o perfil afetado. Cache de testes precisa dessas entradas ou não pode fornecer evidência de aprovação.

No desenvolvimento, executar a suíte da biblioteca e consumidores afetados para feedback rápido. Para homologação final, preservar toda a sequência obrigatória do AGENTS e os perfis exigidos pelo escopo do produto. Assim, isolamento reduz custo de manutenção sem esconder falhas de integração.

## 11. Prova de responsividade

Instrumentação a implementar antes de declarar o novo runtime aprovado:

- Main: atraso do event loop, utilização, duração dos handlers e round-trip de uma rota leve de health.
- Renderer: long tasks, resposta a interação, tempo de frame e glitches de áudio.
- Executor: fila em itens/bytes, idade da tarefa, duração, cancelamentos, timeouts, reinícios, CPU e RSS.
- Produto: latência até primeiro texto/áudio e tempo entre detecção de interrupção e parada do playback.

Medir com monitorEventLoopDelay/eventLoopUtilization e perfis de CPU apropriados ao processo. Referência: [perf_hooks do Node](https://nodejs.org/api/perf_hooks.html). Contagem de frames e eventos de áudio complementa a percepção visual; screenshots sozinhos não provam fluidez.

Procedimento reproduzível:

1. Registrar SHA, build, SO, CPU, RAM, GPU, modelo, tamanho do banco, threads nativas e modo de energia.
2. Coletar baseline ocioso e sob uso normal antes da migração.
3. Aquecer por 30 segundos; medir três execuções de 5 minutos por cenário. Medir cold start separadamente, sem aquecimento.
4. Combinar conversa, STT/TTS, avatar e captura. Adicionar indexação/exportação para testar contenção; repetir durante jogo representativo.
5. Injetar tarefa lenta, fila cheia, filho sem resposta, crash e cancelamento. Continuar interagindo com a janela.
6. Registrar N, p50/p95/p99 e máximo; anexar traces e logs sanitizados por cenário.
7. Repetir após a migração no mesmo ambiente e comparar responsividade, RAM e latência da tarefa.

Alvos iniciais de POC, ainda não homologados: round-trip de health p95 até 100 ms e p99 até 250 ms; atraso do event loop do Main p99 até 50 ms; nenhum bloqueio de 500 ms atribuível à tarefa migrada; nenhuma fila acima de seu teto configurado; nenhuma resposta obsoleta aplicada após cancelamento/reinício. Revisar esses limites com a baseline e publicar qualquer alteração com justificativa, sem ajustá-los apenas para fazer um resultado passar.

Uma amostra de 5 minutos não prova ausência de vazamento. Executar adicionalmente soak de 30 minutos com carga repetível e ciclos de ativação/desativação; conferir listeners/pendências zerados e memória estabilizada após aquecimento. Windows, Linux e macOS precisam de evidência própria para captura, áudio, subprocessos e empacotamento. Um resultado Windows não homologa as outras plataformas.

## 12. Passo a passo da migração

### Etapa 0 — baseline e contrato de adoção

1. Registrar worktree e preservar mudanças de terceiros.
2. Inventariar API, config, eventos, dados, UI, prompts e recursos de cada módulo.
3. Capturar os fluxos críticos atuais e medir o cenário da seção 11.
4. Separar defeitos conhecidos de comportamento a preservar.
5. Fechar a matriz de ownership (4.1) e a matriz de requisitos/oráculos (10.1) antes de extrair o piloto.

Saída: matriz de baseline com evidência, risco e dono. Não mover código nesta etapa.

### Etapa 1 — scheduler por instância, no lugar atual

1. Caracterizar agendamento, recorrência, restart, falha de LLM e cancelamento.
2. Introduzir a fábrica e injetar relógio, repositório, fala e notificação.
3. Retirar estado global do serviço, store-ref e tools; criar tools ligadas à instância.
4. Garantir rollback de ativação parcial e dispose idempotente.
5. Montar duas instâncias simultâneas no teste de biblioteca.

Saída: mesma capacidade, sem singleton mutável. Aceite: os testes da seção 10 e fluxo real de lembrete continuam passando. Rollback: composição anterior, sem mudança de schema nesta etapa.

### Etapa 2 — pacote piloto e fronteiras executáveis

1. Extrair scheduler para o pacote proposto, mantendo API e comportamento.
2. Declarar exports, dependências e comandos de build/typecheck/test.
3. Preservar localização relativa de prompts e empacotá-los.
4. Atualizar roster explícito do Main e UI para consumir o pacote.
5. Criar teste consumidor externo dos exports construídos.
6. Atualizar AGENTS/specs e gates para a nova fronteira. A regra antiga de ports com container não se aplica silenciosamente ao novo layout.
7. Aplicar a nomenclatura da seção 5.1 ao pacote piloto e verificar imports em ambiente com filesystem sensível a caixa.

Saída: scheduler testa sem importar apps/desktop e funciona pelo app. Rollback: selecionar a composição anterior; nunca ativar as duas implementações ao mesmo tempo.

### Etapa 3 — fechar provas e governança

1. Separar no homologate bibliotecas headless de adaptadores Electron, com requisitos apropriados para cada classe.
2. Para bibliotecas, reprovar ausência de suíte, falha de import público e falta de teste de montagem.
3. Substituir hash de contrato como evidência única por testes de compatibilidade e comportamento. Hash continua útil para detectar alteração, não para garantir correção.
4. Adicionar os gates da seção 13 aos comandos obrigatórios.
5. Produzir o relatório por perfil e implementar invalidação por artefato/dependências (10.3), sem reaproveitar aprovação apenas pelo hash da API.

Saída: uma violação criada deliberadamente reprova o comando correspondente; removê-la restaura o resultado.

### Etapa 4 — proteger execução e filas

1. Instrumentar Main, renderer e utility existente.
2. Corrigir as mensagens contraditórias de fallback em inferência.
3. Limitar fila, bytes, concorrência, prazo e respostas por geração.
4. Testar hang, crash, reinício e cancelamento real, incluindo a limitação de ONNX já iniciado.
5. Extrair executor adicional somente se o benchmark justificar. Mover execução em alteração separada da extração de biblioteca.

Saída: relatório comparativo da seção 11 e degradação visível. Rollback: configuração anterior segura; nenhuma volta de inferência ao Main.

### Etapa 5 — features restantes

Aplicar o padrão, uma biblioteca por vez: mood e user; character; person-catalog; gamer-companion; desktop e browser; mcp e privacy. Ajustar a ordem quando um contrato obrigatório ainda não estiver disponível, mantendo adaptador de compatibilidade temporário com dono e critério de remoção.

Para cada uma: extrair estado → provar duas instâncias → fechar contratos → empacotar → integrar UI/IPC → validar falhas → migrar ownership dos dados, se necessário. Telemetry e update seguem a mesma disciplina de lifecycle e testes de adaptador.

Saída por biblioteca: API documentada, dado com dono, dependências fechadas, capacidade utilizável no app e rollback ensaiado.

### Etapa 6 — conversation, memory, llm, voice e vision

1. Consolidar a fronteira de conversa hoje dividida entre brain e chat sem alterar comportamento de turno.
2. Migrar memória por contratos explícitos, retirando buscas genéricas de serviço.
3. Extrair LLM preservando ZDR, offline, aprovação e semântica de failover.
4. Extrair voice e vision preservando PCM16 mono 16 kHz no Main e APIs web no renderer.
5. Retirar implementações do SDK após migrar todos os consumidores.
6. Remover fachadas temporárias somente quando grafo e testes demonstrarem ausência de uso.

Saída: conversa completa por texto e voz, memória, contexto de jogo e tools passam juntos. Não considerar inferência, áudio ou GPU homologados com testes apenas headless.

### Etapa 7 — aceitação do produto e encerramento

Executar todos os gates obrigatórios, cenários de responsividade, recuperação, empacotamento e matriz por SO. Verificar cada capacidade do inventário da etapa 0, incluindo telas, botões, toggles e estados degradados. Registrar pendências como pendências; não encerrar a migração com feature omitida ou teste substituído por mock de sua implementação.

## 13. Gates novos e onde entram

São entregáveis futuros da migração. Este documento não instala gates nem declara suas proteções ativas. Não faz sentido fazê-los reprovar imediatamente toda a árvore antiga antes de extrair o piloto; a adoção terá escopo explícito por pacote, sem permitir novas violações nos pacotes migrados.

| Invariante proposto                                               | Automação a entregar                                                                                                       | Passo obrigatório                                   |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Pacote não importa app/interior de outro pacote; grafo sem ciclos | Resolução de imports e manifestos, incluindo dinâmicos literais e aliases; dinâmicos não resolvíveis exigem roster fechado | pnpm gates                                          |
| Renderer não alcança Node transitivamente                         | Inspeção do grafo e bundle de cada entrypoint UI                                                                           | pnpm gates e build                                  |
| Dependências e exports completos                                  | Consumidor externo do pacote construído e typecheck                                                                        | typecheck/build e contrato de empacotamento no E2E  |
| Instâncias independentes e recursos liberados                     | Testes comportamentais A/B, start/dispose e ausência de efeitos de import                                                  | pnpm test:unit                                      |
| Capacidade ausente não aparece pronta                             | Contrato de ativação/health e tela degradada                                                                               | test:unit e pnpm test                               |
| Fila limitada e resposta obsoleta rejeitada                       | Testes de lotação, geração, deadline e cancelamento                                                                        | pnpm test:unit                                      |
| Filho não bloqueia a UI sob carga                                 | Cenário instrumentado de Electron e injeção de falha                                                                       | pnpm test                                           |
| Payload/config do painel alcança função real                      | Contrato schema/handler e fluxo Playwright                                                                                 | test:unit e pnpm test                               |
| Assets de prompt e executores presentes                           | Inventário do bundle e inicialização empacotada                                                                            | build e pnpm test                                   |
| Nomes e entrypoints seguem a convenção adotada                    | Gate de nomenclatura com exceções nominadas e prova de imports sensíveis a caixa                                           | pnpm gates e typecheck em Linux                     |
| Boot/lifecycle não depende de feature alheia nem vaza recursos    | Testes de start/dispose concorrentes, revogação e capacidade opcional ausente                                              | test:unit e pnpm test                               |
| Contratos de UI/IPC têm dono e autorização real                   | Conferência de schema/rotas e chamadas por superfície permitida/negada                                                     | gates, test:unit e pnpm test                        |
| Homologação pertence ao artefato atual                            | Conferir identidade do bundle, dependências, critérios e validade do relatório                                             | Encerramento de pnpm test no fluxo de homologação   |
| Asserções detectam defeitos críticos deliberados                  | Mutação seletiva e propriedades da seção 10.2; ferramenta e baseline ainda pendentes                                       | pnpm test:unit no perfil obrigatório de homologação |

Todo gate estático novo terá probe negativo. Uma métrica de hardware não pode ser provada por regex; precisa do cenário de runtime correspondente. Jobs dependentes de GPU, modelos ou outro SO devem estar provisionados no pipeline de homologação; ausência do runner não vira skip aprovado.

## 14. Comandos atuais e validação

Comandos existentes, executados da raiz, na ordem do AGENTS. O documento não altera seus scripts:

```powershell
pnpm -s lint
pnpm -s gates
pnpm -s typecheck
pnpm -s test:unit
pnpm -s build
pnpm -s test
```

Parar a sequência quando houver falha, investigar e registrar o bloqueio. Mudou UI: abrir os PNGs de `apps/desktop/e2e/screenshots/`. O comando de E2E atual também dispara build; preservar o fluxo oficial até uma mudança explícita no workflow.

Na implementação, anexar por etapa: SHA, comandos e exit codes, testes e cenários efetivamente cobertos, screenshots quando houver UI, métricas e procedimento de rollback. Registrar o resumo no checkpoint da seção 17, distinguindo evidência de código, teste isolado e homologação de runtime.

## 15. Alternativas e decisão

| Alternativa                                                          | Avaliação                                                                      |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Apenas mover diretórios para packages                                | Mantém singletons e dependências ocultas; insuficiente                         |
| Processo para toda feature                                           | Aumenta RAM, transporte e supervisão sem benefício demonstrado para I/O leve   |
| EventBus para qualquer interação                                     | Dificulta resposta, confirmação e diagnóstico de comandos                      |
| Reescrever todo o app de uma vez                                     | Mistura mudança de fronteira com mudança de comportamento e dificulta rollback |
| Bibliotecas por domínio, estado por instância e executores seletivos | Recomendada: permite provar cada limite e migrar por capacidade completa       |

Adoção exige atualizar as specs atuais nas etapas indicadas. Nenhuma promessa de ausência absoluta de falhas acompanha a arquitetura: o aceite é a evidência de isolamento, responsividade, degradação e funcionamento do produto sob os cenários definidos.

## 16. Referências

Fontes primárias consultadas em 2026-09-07; confirmar compatibilidade com as versões instaladas antes de adotar APIs novas.

- [Electron: performance e bloqueio do Main](https://www.electronjs.org/docs/latest/tutorial/performance).
- [Electron: modelo de processos](https://www.electronjs.org/docs/latest/tutorial/process-model).
- [Electron: utilityProcess](https://www.electronjs.org/docs/latest/api/utility-process).
- [Node: Worker Threads](https://nodejs.org/api/worker_threads.html).
- [Node: medição de performance](https://nodejs.org/api/perf_hooks.html).
- [Node: exports e limites de encapsulamento](https://nodejs.org/api/packages.html#main-entry-point-export).
- [Vitest: projetos de teste](https://vitest.dev/guide/projects).
- [Angular v18: nomenclatura clássica](https://v18.angular.dev/style-guide/#style-02-02).
- [Angular: guia atual e consistência por feature](https://angular.dev/style-guide).
- [Vitest: cobertura](https://vitest.dev/guide/coverage).
- [Stryker: estados e métricas de mutação](https://stryker-mutator.io/docs/mutation-testing-elements/mutant-states-and-metrics/).
- [Arquitetura atual](../specs/01-architecture.md).
- [Gates atuais](../specs/11-quality-gates.md).

## 17. Checkpoint de implementação e adaptação

Última inspeção: 2026-09-07, HEAD `98cc1493`. Este checkpoint registra o worktree inspecionado, não apenas a intenção do plano. Na segunda revisão, a versão anterior do arquivo estava no stage; os ajustes desta revisão foram feitos apenas no worktree, preservando o stage existente.

**Situação geral:** etapas 1 a 7 executadas. O scheduler virou biblioteca por instância em `packages/scheduler`, com lifecycle, health e degradação declarada; a homologação passou a rodar por perfil e a produzir relatório vinculado ao artefato. Nenhuma etapa está homologada com runtime empacotado — build e E2E ficam para a etapa 7, por decisão explícita de escopo desta execução.

### 17.1 Legenda

- **Existente:** implementação anterior confirmada no código; não implica validação completa nesta sessão.
- **Parcial:** atende uma parte do objetivo, mas ainda tem lacunas explícitas.
- **Pendente:** entregável do plano ainda não implementado.
- **Não validado:** falta evidência de execução para afirmar o aceite, mesmo quando há código.

### 17.2 O que já existe e o que falta adaptar

| Item                                                          | Estado                          | Evidência atual                                                                                                                                                                                                                                  | Adaptação ou prova restante                                                                                                                                                                         |
| ------------------------------------------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plano de arquitetura, processos, testes e rollback            | Documentado nesta sequência     | Seções 1–16 deste arquivo                                                                                                                                                                                                                        | Executar as etapas; documentação não equivale a implementação                                                                                                                                       |
| Roster explícito de features e UI                             | Existente                       | ~~`apps/desktop/src/main/infrastructure/features.ts`~~ e `apps/desktop/src/renderer/src/lib/feature-ui-registry.ts`                                                                                                                              | Apontar para bibliotecas extraídas, preservando conferência nos dois sentidos                                                                                                                       |
| Manifests e gates de fronteira/container                      | Parcial                         | `scripts/checks/module-manifest.mts`, `scripts/checks/boundaries.mts` e ~~`scripts/checks/container-seams.mts`~~                                                                                                                                 | Cobrir grafo transitivo e entrypoints de pacotes; resolver inconsistências registradas abaixo                                                                                                       |
| Portas explícitas no brain                                    | Existente                       | ~~`apps/desktop/src/main/services/brain/ports.ts`~~ só tem portas nomeadas e tipadas; ~~`apps/desktop/src/main/services/brain/__tests__/brain-ports.contract.test.ts`~~ prova ausência, injeção sem container, troca por remontagem e falha alta | O `active` continua sendo módulo-nível por ser a raiz de composição do processo Main; a troca é provada por teste, não por instância paralela                                                       |
| Scheduler como biblioteca por instância                       | Existente                       | ~~`packages/scheduler/src/services/scheduler.runtime.ts`~~ (fábrica, lifecycle e health) e ~~`packages/scheduler/src/__tests__/scheduler.runtime.spec.ts`~~ (A/B e lifecycle)                                                                    | Falta a UI: `apps/desktop/src/features/scheduler/renderer/settings.tsx` continua no app                                                                                                             |
| Pacotes de domínio independentes                              | Parcial                         | `@mika/scheduler`, `@mika/mood` e `@mika/llm`, cada um com exports, typecheck, suíte e homologação próprios                                                                                                                                      | Features que tocam Electron ou serviços do Main ficaram no app com perfil `adapter`; extraí-las exige antes o perfil de integração Desktop                                                          |
| SDK com superfície mínima                                     | Parcial                         | O gateway de LLM saiu para `packages/llm`; a fachada `createRuntime` (~~packages/sdk/src/utils/runtime.ts~~) foi removida depois do grafo provar zero consumidores                                                                               | `packages/sdk/package.json` ainda tem exports curingas; `llm-catalog-data.ts` e `llm-constants.ts` continuam no SDK porque `contracts/types` e o renderer os importam, e movê-los fecharia um ciclo |
| Testes headless                                               | Parcial                         | `vitest.config.mts` tem ambiente Node e seleção global                                                                                                                                                                                           | Projetos por biblioteca, testes da API completa e contratos de adaptadores                                                                                                                          |
| Homologação individual                                        | Existente                       | `scripts/homologate.mts` roda por perfil (`scripts/homologation/profile.mts`); biblioteca sem suíte, sem import público ou sem teste de montagem reprova                                                                                         | Falta o perfil de integração Desktop (UI/IPC/empacotamento)                                                                                                                                         |
| Inferência em utilityProcess                                  | Existente                       | ~~`apps/desktop/src/main/services/inference/client.ts`~~ cria processo auxiliar                                                                                                                                                                  | Medir responsividade sob carga e recuperação real; não extrapolar o resultado para todas as features                                                                                                |
| Proteção contra fallback pesado no Main                       | Existente                       | O método `run` do client rejeita trabalho sem host                                                                                                                                                                                               | Corrigir logs contraditórios que ainda anunciam fallback ao Main                                                                                                                                    |
| Timeout e limite de reinício do utility                       | Existente                       | ~~`apps/desktop/src/main/services/inference/__tests__/inference-client.spec.ts`~~ prova prazo de prontidão, queda, reinício e rajada                                                                                                             | Falta estado visível ao usuário: hoje a degradação só aparece no log                                                                                                                                |
| Cancelamento de inferência                                    | Existente                       | ~~`apps/desktop/src/main/services/inference/__tests__/dispatcher.spec.ts`~~ prova descarte antes de iniciar e silêncio quando o motor já rodava                                                                                                  | Motor já iniciado segue sem preempção — limitação do ONNX local, registrada e logada                                                                                                                |
| Filas de inferência                                           | Existente                       | ~~`apps/desktop/src/main/services/inference/job-queue.ts`~~ tem teto em itens e bytes; ~~`apps/desktop/src/main/services/inference/dispatcher.ts`~~ aplica lane, prazo e recusa                                                                  | Concorrência medida sob carga real continua pendente (seção 11)                                                                                                                                     |
| Envelope versionado e geração de executor da proposta         | Parcial                         | ~~`apps/desktop/src/main/services/inference/protocol.ts`~~ carrega `generation` e `deadlineAt`; o client descarta resposta de host antigo                                                                                                        | Falta versionar o envelope entre releases e negociar compatibilidade                                                                                                                                |
| Benchmark de Main/renderer, soak e matriz por SO              | Não validado                    | Procedimento especificado na seção 11                                                                                                                                                                                                            | Coletar baseline, traces e resultados por cenário e plataforma                                                                                                                                      |
| Donos de estado, arbitragem de fala e dependências permitidas | Especificado na segunda revisão | Seções 4.1–4.2 e 7.4                                                                                                                                                                                                                             | Implementar as fronteiras e provar concorrência/integração                                                                                                                                          |
| Nomenclatura Angular clássica adaptada a React                | Especificado na segunda revisão | Seção 5.1                                                                                                                                                                                                                                        | Adotar no piloto com atualização coordenada do AGENTS e dos gates                                                                                                                                   |
| Feature sem singleton global                                  | Existente                       | `character`, `person-catalog`, `gamer-companion`, `user` e `privacy` mantêm a instância em `let` na raiz de composição e entregam `getRuntime` às tools; ausência responde `unavailable` nomeado                                                 | Cada suíte prova resolução por chamada e troca por remontagem; nenhuma prova duas instâncias simultâneas no mesmo processo, o que o Main não permite                                                |
| Homologação por critérios, perfil e artefato                  | Parcial                         | `scripts/homologation/report.mts` grava os campos da 10.3 em `docs/homologation/<id>.json`; `scripts/checks/homologation-freshness.mts` invalida por árvore, contrato e dependências                                                             | Falta mutation testing seletivo e o perfil de plataforma/carga                                                                                                                                      |

### 17.3 Andamento por etapa

| Etapa da seção 12                 | Estado                                                                       | Critério para avançar                                                                                                                                                                                                         |
| --------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0 — Baseline e contrato de adoção | Parcial: revisão de código e plano registrados                               | Completar inventário por capacidade e baseline de runtime                                                                                                                                                                     |
| 1 — Scheduler por instância       | Concluída                                                                    | Fábrica, isolamento A/B e lifecycle provados em ~~`packages/scheduler/src/__tests__/scheduler.runtime.spec.ts`~~                                                                                                              |
| 2 — Pacote piloto                 | Concluída                                                                    | `@mika/scheduler` com exports, typecheck e suíte próprios, consumido por ~~`apps/desktop/src/features/scheduler/index.ts`~~                                                                                                   |
| 3 — Provas e governança           | Concluída                                                                    | Perfis, relatório por artefato e `check:homologation-freshness` em `pnpm gates`, com probe negativo                                                                                                                           |
| 4 — Execução e filas              | Parcial: limites, prazo, geração e falhas injetadas entregues e testados     | Falta o relatório comparativo da seção 11, que exige campanha de runtime                                                                                                                                                      |
| 5 — Features restantes            | Concluída                                                                    | Estado global eliminado em character, person-catalog, gamer-companion, user e privacy, cada uma com suíte de contrato; desktop e mcp já não tinham estado global; não existe `features/browser` — browser é serviço de núcleo |
| 6 — Capacidades centrais          | Concluída                                                                    | Fronteira de conversa declarada em dois donos (`chat-turn` no brain, `chat-history` no chat), memória com contrato nomeado de consolidação, LLM extraído para `@mika/llm` e fachada `createRuntime` removida                  |
| 7 — Aceitação do produto          | Parcial: gates, build, E2E e homologação de todos os módulos verdes em win32 | Falta o benchmark da seção 11, a matriz Linux/macOS e o empacotamento por electron-builder                                                                                                                                    |

### 17.4 Evidências de validação e bloqueios conhecidos

Resultados obtidos nesta sequência de revisão/documentação em 2026-09-07; cada um tem escopo limitado:

| Comando ou verificação                                                    | Resultado observado                                                                                                               | Limite da evidência                                                                                     |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `pnpm -s lint`                                                            | Exit 0                                                                                                                            | Biome; warnings pré-existentes fora do escopo                                                           |
| `pnpm -s gates`                                                           | Exit 0 nos 21 gates, incluindo o novo `check:homologation-freshness`                                                              | Estático: lê a árvore, não executa o app                                                                |
| `pnpm -s typecheck`                                                       | Exit 0, 4 tasks                                                                                                                   | Não cobre empacotamento                                                                                 |
| `pnpm -s test:unit`                                                       | Exit 0, 69 arquivos / 457 testes                                                                                                  | Headless: sem Electron, GPU ou áudio                                                                    |
| `pnpm --filter @mika/scheduler -s test`                                   | Exit 0, 2 arquivos / 16 testes                                                                                                    | Prova A/B, lifecycle e degradação; SQLite temporário real                                               |
| `pnpm -s homologate scheduler`                                            | Exit 0, perfil `library`, estado aprovado                                                                                         | Não cobre UI, IPC nem bundle                                                                            |
| `pnpm -s homologate person-catalog`                                       | Exit 0, perfil `adapter`, estado aprovado                                                                                         | Adaptador: headless é informativo por design                                                            |
| Probe negativo de `check:homologation-freshness`                          | Exit 1 ao adulterar o digest da árvore no relatório; exit 0 ao restaurar                                                          | Prova o gate, não a qualidade do relatório                                                              |
| `pnpm -s test:unit` após a etapa 4                                        | Exit 0, 71 arquivos / 473 testes                                                                                                  | Filho de inferência é dublê: prova a máquina de estados do client, não o motor ONNX                     |
| `pnpm -s homologate inference-host`                                       | Exit 0, perfil `adapter`, estado aprovado                                                                                         | `host.ts` continua sem carregar fora do Electron — exceção declarada no manifesto                       |
| `pnpm -s lint` / `gates` / `typecheck` / `test:unit` após as etapas 5 e 6 | Exit 0, 0, 0 e 0 — 22 gates; 74 arquivos / 491 testes                                                                             | Headless: sem Electron, GPU ou áudio                                                                    |
| `pnpm --filter @mika/llm -s test`                                         | Exit 0, 27 testes                                                                                                                 | Prova ciclo de vida da fábrica e isolamento entre instâncias; nenhum provedor real é chamado            |
| `pnpm --filter @mika/mood -s test`                                        | Exit 0, 13 testes                                                                                                                 | Isolamento por instância; sem UI                                                                        |
| `pnpm -s homologate llm-provider`                                         | Exit 0, perfil `library`, estado aprovado                                                                                         | Cobre fronteira, headless, import público, montagem e contrato; não cobre failover real nem ZDR em rede |
| Benchmark comparativo da seção 11                                         | Não executado                                                                                                                     | Exige campanha de runtime com baseline, aquecimento e três execuções por cenário                        |
| `pnpm -s build`                                                           | Exit 0, 2 tasks (electron-vite + Turbo)                                                                                           | Empacotamento com electron-builder não foi exercitado                                                   |
| `pnpm -s test` (E2E Playwright)                                           | Exit 0, 77 passaram / 1 pulado (probe `@probe` de braço), 7,9 min                                                                 | win32, LLM dublê no loopback; a primeira execução reprovou 2 specs e achou um defeito real              |
| Homologação dos 36 módulos                                                | 36 aprovados após a correção; na primeira passada 3 reprovaram (`config-store`, `plugin-runtime`, `tool-registry`)                | Perfis `library` e `adapter`; nenhum cobre UI, IPC ou empacotamento                                     |
| Posse de arquivo (2026-09-19)                                             | 613 de 1293 fontes estavam fora de qualquer `source`; 17 módulos novos levam a 52 aprovados e `check:module-ownership` a 0 órfãos | Cada arquivo pertence ao `source` mais específico; o pai não soma a árvore dos filhos                   |
| Prints do E2E (`apps/desktop/e2e/screenshots/`)                           | Lidos: usuário, privacidade, gamer companion e saúde/desligamento                                                                 | Amostra das features convertidas, não os 88 arquivos                                                    |
| `homologate:features` com Ollama local (`qwen3.5:4b`) — matriz do roster  | 26 verificações: 25 ok, 1 sem credencial (MCP sem servidor), 0 falhas                                                             | Modelo de 4B numa máquina só; prova que a tool responde, não que a resposta é boa                       |
| `features-core.spec.ts` com o mesmo modelo — núcleo, visão e voz          | 29 verificações, 0 falhas, com captura de tela ligada                                                                             | A tela capturada era a de bloqueio do Windows; a visão descreveu o que havia                            |
| `check:tool-coverage`                                                     | Exit 0, 22 tools reconhecidas e todas citadas em teste                                                                            | Prova que existe teste com aquele nome, não que o teste é bom                                           |
| `tsc --noEmit -p apps/desktop/tsconfig.e2e.json`                          | Exit 0 após corrigir 29 erros que nunca haviam sido checados                                                                      | A árvore de E2E estava fora do `pnpm typecheck` desde sempre                                            |

Bloqueios resolvidos: os dois manifestos congelados divergiam por mudanças reais de contrato. Em inferência a causa era um `import * as os` morto deixado pelo commit `47408f55`, removido em vez de ter o hash regravado; em person-catalog a mudança veio da correção de colisão de alias já commitada, e o hash foi reaberto com a data de hoje.

Pendências registradas ao fim da etapa 7, na ordem em que doem: o benchmark comparativo da seção 11 continua não executado — exige campanha de runtime com baseline e três execuções por cenário; a matriz por SO só tem win32, porque é a única plataforma disponível nesta máquina; o empacotamento por electron-builder não foi exercitado (`pnpm build` cobre bundle, não instalador), e E2E e `build:win:dir` não convivem na mesma pasta de resultados; e o perfil de integração Desktop (UI, IPC, bundle) segue sem existir, o que mantém toda feature acoplada ao Electron no perfil `adapter`.

Limites conhecidos que continuam abertos: a UI do scheduler (`apps/desktop/src/features/scheduler/renderer/settings.tsx`) não foi extraída, porque depende de quatro internos do renderer do desktop; e o critério de montagem do perfil `library` é heurístico — hoje procura `new X(`, `createX(` ou `.start(` junto de `.dispose(` ou `.stop(` na suíte do módulo, o que detecta ausência de teste de ciclo de vida, não a qualidade dele. A versão anterior só aceitava `.start(` e reprovava biblioteca cuja montagem é uma fábrica, como `@mika/llm`.

### 17.5 Próxima execução e atualização do checkpoint

Próximo trabalho: etapa 4 — instrumentar Main/renderer/utility, limitar a fila de inferência (itens, bytes, prazo e geração) e testar hang, crash e cancelamento real.

Ao finalizar cada alteração de implementação:

1. Registrar data, SHA e mudanças ainda não commitadas relevantes.
2. Atualizar somente os itens realmente alterados, separando código anterior de adaptação nova.
3. Anexar comandos, exit codes e escopo dos testes; manter como não validado o que não foi executado.
4. Registrar falhas, limitações de ambiente, evidências de runtime e rollback aplicável.
5. Marcar etapa concluída somente quando seu critério de aceite estiver provado; registrar o próximo passo concreto.

Modelo para as próximas entradas, a preencher com resultados reais:

```text
Data / SHA / estado do worktree:
Etapa e itens adaptados:
Arquivos e contratos afetados:
Verificações executadas e resultados:
Evidência de runtime e plataformas cobertas:
Pendências / bloqueios:
Rollback:
Próximo passo:
```

Entradas registradas:

```text
Data / SHA / estado do worktree: 2026-09-07 / 0aa999c4 / limpo após o commit
Etapa e itens adaptados: 1 e 2 — scheduler por instância e pacote piloto @mika/scheduler
Arquivos e contratos afetados: packages/scheduler/** (novo), apps/desktop/src/features/scheduler/index.ts, apps/desktop/electron.vite.config.ts, apps/desktop/build/prompt-dirs.ts, scripts/checks/lib.mts e 12 gates
Verificações executadas e resultados: lint 0; gates 0; typecheck 0; test:unit 0 (457); @mika/scheduler 0 (16)
Evidência de runtime e plataformas cobertas: nenhuma — headless em win32; build e E2E reservados para a etapa 7
Pendências / bloqueios: UI do scheduler continua em apps/desktop
Rollback: reverter o commit restaura a feature monolítica; nenhum dado migrou de banco
Próximo passo: etapa 3
```

```text
Data / SHA / estado do worktree: 2026-09-07 / a partir de 0aa999c4 / worktree com a etapa 3
Etapa e itens adaptados: 3 — perfis de homologação, relatório por artefato e gate de validade
Arquivos e contratos afetados: scripts/homologate.mts, scripts/homologation/**, scripts/checks/homologation-freshness.mts, packages/sdk/src/modules/define-module.ts, package.json
Verificações executadas e resultados: lint 0; gates 0 (21 gates); typecheck 0; homologate scheduler e person-catalog aprovados; probe negativo reprova e restaura
Evidência de runtime e plataformas cobertas: nenhuma — o relatório é headless por definição
Pendências / bloqueios: sem perfil de integração Desktop e sem mutation testing
Rollback: remover o gate da lista de `gates` e apagar `docs/homologation/`
Próximo passo: etapa 4
```

```text
Data / SHA / estado do worktree: 2026-09-07 / a partir de f33b5682 / worktree com a etapa 4
Etapa e itens adaptados: 4 — teto de fila, prazo, geração, instrumentação do renderer e provas de falha
Arquivos e contratos afetados: inference/job-queue.ts, inference/dispatcher.ts (novo), inference/host.ts, inference/client.ts, inference/protocol.ts, renderer/src/lib/long-task-probe.ts (novo), renderer/src/main.tsx
Verificações executadas e resultados: lint 0; gates 0; typecheck 0; test:unit 0 (473, +16 novos); homologate inference-host aprovado
Evidência de runtime e plataformas cobertas: nenhuma — as falhas são injetadas com dublê do utilityProcess; o motor ONNX não é exercitado
Pendências / bloqueios: relatório comparativo da seção 11 não executado; motor já iniciado continua sem preempção; degradação ainda não é visível na UI
Rollback: reverter o commit devolve a fila ilimitada e o protocolo sem geração/prazo; nenhum dado persistido muda
Próximo passo: etapa 5
```

```text
Data / SHA / estado do worktree: 2026-09-07 / a partir de f33b5682 / worktree com a etapa 5
Etapa e itens adaptados: 5 — fim do singleton global em character, person-catalog, gamer-companion, user e privacy
Arquivos e contratos afetados: features/{character,person-catalog,gamer-companion,user,privacy}/{index.ts,*.feature.ts,tools/**,services/*.runtime.ts,__tests__/**}, packages/mood/**, manifestos recongelados dos módulos tocados
Verificações executadas e resultados: lint 0; gates 0 (22); typecheck 0; test:unit 0 (491); @mika/mood 0 (13)
Evidência de runtime e plataformas cobertas: nenhuma — headless em win32; build e E2E reservados para a etapa 7
Pendências / bloqueios: as features acima ficaram no app com perfil `adapter` em vez de virarem pacote, porque dependem de Electron e de serviços do Main; empacotá-las exige antes o perfil de integração Desktop
Rollback: reverter os commits da etapa devolve o runtime de módulo; nenhum dado persistido muda
Próximo passo: etapa 6
```

```text
Data / SHA / estado do worktree: 2026-09-07 / 193b86ee / limpo após o commit
Etapa e itens adaptados: 6 — fronteira de conversa declarada, memória por contrato nomeado, LLM extraído e fachada removida
Arquivos e contratos afetados: packages/llm/** (novo), packages/sdk/src/core/container.ts, packages/sdk/src/index.ts, ~~packages/sdk/src/utils/runtime.ts~~ (removido), apps/desktop/src/main/bootstrap.ts, apps/desktop/electron.vite.config.ts, main/services/memory/ports.ts, main/services/brain/ports.ts, brain/__tests__/brain-ports.contract.test.ts (novo), scripts/homologation/criteria.mts
Verificações executadas e resultados: lint 0; gates 0 (22); typecheck 0; test:unit 0 (491); @mika/llm 0 (27); homologate llm-provider aprovado (perfil library)
Evidência de runtime e plataformas cobertas: nenhuma — nenhum provedor de LLM é acionado nos testes, por regra de custo zero
Pendências / bloqueios: voice e vision não foram empacotados — já têm portas nomeadas injetáveis e manifesto, e são adaptadores de Electron, então o empacotamento não se aplica; `llm-catalog-data.ts` e `llm-constants.ts` continuam no SDK para não fechar ciclo com `contracts/types` e o renderer
Rollback: reverter os commits da etapa devolve o LLM ao SDK e as buscas genéricas por token; o container volta a construir a fábrica internamente
Próximo passo: etapa 7 — aceitação, com build, E2E e homologação de todos os módulos
```

```text
Data / SHA / estado do worktree: 2026-09-07 / 98cc1493 / limpo após o commit
Etapa e itens adaptados: 7 — aceitação: gates, build, E2E, homologação dos 36 módulos e leitura de prints
Arquivos e contratos afetados: apps/desktop/src/features/user/{index.ts,user.module.ts}, packages/sdk/src/{index.ts,plugins,services,tools}/**, scripts/homologation/criteria.mts, docs/homologation/*.json
Verificações executadas e resultados: lint 0; gates 0 (22); typecheck 0; test:unit 0 (77 arquivos / 500 testes); build 0; E2E 0 (77 passaram, 1 pulado); 36 de 36 módulos aprovados
Evidência de runtime e plataformas cobertas: win32 apenas, com Electron real e LLM dublê no loopback; prints de usuário, privacidade, gamer companion e desligamento conferidos
Pendências / bloqueios: benchmark da seção 11 não executado; sem matriz Linux/macOS; sem prova de empacotamento por electron-builder; sem perfil de integração Desktop; UI do scheduler ainda no app
Rollback: cada etapa reverte por commit; a única mudança de comportamento em produção é a capacidade `llm:tools` do perfil de usuário
Próximo passo: benchmark da seção 11 e perfil de integração Desktop — o que sobrou exige campanha de runtime, não refatoração
```

A etapa 7 achou dois defeitos que gate nenhum via. O perfil de usuário passou a registrar tool na ativação sem pedir `llm:tools`, o loader recusou e o app subia degradado, sem `user_profile` — lint, gates, typecheck e unidade seguiam verdes, e só o E2E reprovou. E três módulos do SDK reprovaram a homologação por não declararem superfície pública nem provarem desligamento; a correção virou barril por módulo e suíte de ciclo de vida, e uma dessas asserções é exatamente a do primeiro defeito.

### 17.6 Homologação com modelo local (2026-09-08)

A homologação passou a rodar contra o Ollama local com `qwen3.5:4b` (3,4 GB; `tools`, `vision` e
`thinking` na mesma pesagem) e `nomic-embed-text` para embeddings. Das 22 tools registradas, a
matriz cobria 10; `apps/desktop/e2e/homologation/features-core.spec.ts` cobre as outras 12 e
`tool-routing.spec.ts` põe o próprio modelo escolhendo a ferramenta — o que o Ollama dublê, por
construção, não consegue provar.

Isso achou dois defeitos que lint, gates, typecheck, unidade e o E2E determinístico não viam:

1. O caminho do Ollama nunca lia `llm.enableThinking`. Modelo que pensa gasta o orçamento inteiro de
   saída no `thinking` e devolve `content` vazio com `done_reason: "length"`. Agora vai
   `think: false` — e só o `false`, porque `think: true` num modelo sem a capacidade responde 400.
2. Toda tool que gera texto embrulhava esse vazio em `success: true`: `{"recap":""}`,
   `{"guide":""}`, `analysis: ""`. A Mika anunciava o que não tinha feito. `generateRaw` tenta de
   novo e falha alto; visão e guia de jogo recusam a resposta vazia.

O gate `check:tool-coverage` fecha a porta: tool no catálogo do LLM que nenhum teste chama reprova.
E `apps/desktop/tsconfig.e2e.json` põe a árvore de E2E no typecheck, que ela nunca tinha visto —
os 29 erros que estavam lá foram corrigidos.

O `tool-routing.spec.ts` também mostrou o limite de um modelo de 4B: 1 acerto em 5, e nos erros ela
ANUNCIA a ação que não executou ("Agendei para daqui a 40 minutos" sem nenhuma chamada de
`reminder`). Não é o caminho de ferramentas do app — a mesma pergunta, mandada direto ao `/api/chat`
do Ollama com só a definição de `reminder`, volta com o `tool_call` e os argumentos certos. O que
muda é o prompt inteiro: 22 definições (~7,3k tokens) mais o `system.md`. É exatamente a hipótese
que a seção 11 e o `prompt-benchmark.spec.ts` existem para medir, e continua pendente de campanha.


### 17.7 Homologação em três modelos: 27B em 1 bit e OpenRouter (2026-09-08)

O mesmo roteiro passou a rodar contra qualquer provedor pelo `MIKA_HOMOLOG_PROVIDER` /
`MIKA_HOMOLOG_MODEL`, e cada rodada grava relatório próprio (`MIKA_HOMOLOG_RUN`). Trocar de modelo é
o teste que expõe o que o modelo antigo escondia — foi assim nas três rodadas abaixo.

| Rodada                                  | Suítes | features + core           | Roteamento |
| --------------------------------------- | ------ | ------------------------- | ---------- |
| `ollama · qwen3.5:4b`                   | 18/18  | 55 verificações, 0 falhas | 1/5        |
| `ollama · qwen3.8:27b` UD-IQ1_S (1 bit) | 18/18  | 55 verificações, 2 falhas | 1/5        |
| `openrouter · google/gemma-4-31b-it`    | 18/18  | 55 verificações, 0 falhas | **5/5**    |

Três defeitos reais, todos verdes em lint, gates, typecheck, unidade e E2E determinístico:

1. **O app nunca pedia `num_ctx` ao Ollama.** Sem a opção o servidor assume 4096, e o prompt real da
   Mika mede 12,3k tokens — medido depois em 26,5k com resultados de ferramenta no meio. Com o 27B
   TODO turno de chat morria em `400 exceed_context_size_error`. Com o 4B passava despercebido
   porque o servidor cabia o modelo inteiro na VRAM e escolhia sozinho uma janela maior: verde por
   sorte de memória. `ollamaContextSetting` pede `min(janela real, 32768)`, e `llm-catalog.ts`
   passou a ler `*.context_length` do `/api/show`.
2. **Esse 400 era classificado como `unknown`**, e a tela dizia "Erro com Ollama. Verifica Settings →
   LLM" para um problema de contexto — escondendo a única ação que resolve. Agora é `token_limit`.
3. **A recusa por tamanho no laço de ferramentas não poupava nada**: `capForModel` recusava
   resultado acima de 48.000 caracteres e anexava um `preview` de… 48.000 caracteres. Quem estourou
   foi `memory_console(snapshot)`, que devolve o `systemPrompt` inteiro mais toda a memória episódica
   de volta para dentro do prompt. O recorte da recusa agora é de 2.000.

E dois buracos nos próprios testes, que davam verde sem provar nada:

- O IPC `chatSend` não devolvia `role`, então "Erro com Ollama." chegava ao chamador
  indistinguível de uma resposta. A homologação de roteamento leu cinco erros como cinco respostas
  válidas. O contrato agora carrega `role`, e os dois specs reprovam turno `system`.
- O `prompt-benchmark` fixava `provider: 'ollama'`: na rodada de OpenRouter ele mandou o slug remoto
  para o Ollama local e as duas variantes "passaram" em 4 segundos, sem uma resposta sequer.

Com número de verdade, o benchmark do `system.md` (`google/gemma-4-31b-it`, 3 tentativas por caso):
variante A, com o roteamento em prosa, 12/15 acertos e 20,2k tokens de prompt médio; variante B, sem
a prosa, 10/15 e 16,5k. A prosa custa ~3,7k tokens por turno e compra 2 acertos em 15.

Sobre o 1 bit: o `qwen3.8:27b` em UD-IQ1_S (7,1 GB) roda e responde, mas paga o preço na saída —
vaza o andaime do raciocínio como texto do usuário ("**Thought:** 1. The user is asking…") e escreve
a chamada de ferramenta como PROSA (`<function_calls> <parameter=reminder>…`) em vez de emitir
`tool_call`, o que nenhum sanitizador de tag pega. Um `recommend_media` não voltou JSON parseável.
O acerto de roteamento de 5/5 do modelo remoto contra 1/5 dos dois locais fecha a questão aberta na
17.6: o caminho de ferramentas do app está certo; o que faltava era capacidade do modelo.

Aviso que fica: com teto de 32k de janela e resultado de ferramenta valendo até 48 mil caracteres,
dois resultados grandes no mesmo turno ainda estouram o contexto local. O usuário vê a mensagem
certa ("Contexto muito longo"), mas não tem onde ajustar a janela — não há campo de `num_ctx` nas
Settings.
