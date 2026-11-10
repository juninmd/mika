# Contrato de Licença de Usuário Final (EULA) — Mika

> **RASCUNHO — requer revisão jurídica antes da publicação.**
> Campos entre colchetes (`[RAZÃO SOCIAL]`, `[E-MAIL DE SUPORTE]` e outros) são placeholders: nenhum
> dado da empresa foi inventado. Vigência: `[DATA DE VIGÊNCIA]`.

Este contrato é entre você ("você") e **[RAZÃO SOCIAL]** ("nós"), sobre o uso do aplicativo Mika. Ele
complementa o texto de licença do arquivo LICENSE, a Política de Privacidade (PRIVACY.md) e os avisos
de terceiros (THIRD-PARTY-NOTICES.md). Se você compra o Mika pela Steam, o Acordo de Assinante Steam
(Steam Subscriber Agreement) rege a compra; este EULA rege o uso do software.

Ao instalar ou usar o Mika, você declara ter lido e aceitado estes termos. Se não aceita, não instale
nem use. O aplicativo mostra uma tela de aceite no primeiro uso (e quando estes textos mudam), que
exige também a confirmação de que você tem 18 anos ou mais.

## 1. O que o Mika é

O Mika é uma companheira de IA com avatar (VTuber) para games, código e cultura nerd. Roda no seu
computador. **A personagem é fictícia e gerada por software**; não é uma pessoa, não tem consciência
e não substitui contato humano, profissionais de saúde ou aconselhamento de qualquer tipo.

## 2. Licença

Concedemos uma licença pessoal, não exclusiva, intransferível, não sublicenciável e revogável para
instalar e usar o Mika em dispositivos que você possui ou controla. O Mika é licenciado, não vendido.
Os termos completos e as restrições estão no arquivo LICENSE.

Em resumo, você **não pode**: copiar ou modificar o Mika; fazer engenharia reversa, descompilar ou
extrair código, prompts, modelos ou assets (exceto onde a lei proíbe esta restrição); redistribuir,
alugar ou revender; remover avisos de direitos; contornar proteções técnicas; usar o Mika para violar
a lei ou direitos de outras pessoas.

## 3. Conteúdo gerado por IA ao vivo

Tudo o que o Mika diz, escreve ou faz é **gerado ao vivo por modelos de linguagem**, no momento da
conversa. Por isso:

- a saída é probabilística e **pode estar errada, incompleta, desatualizada ou ofensiva**;
- ela **não é aconselhamento** médico, jurídico, financeiro, de saúde mental ou profissional;
- você é responsável por conferir a saída e por qualquer decisão ou uso que fizer dela;
- a saída pode se assemelhar a conteúdo existente; não garantimos que seja original nem livre de
  direitos de terceiros;
- não reivindicamos propriedade sobre a saída que o Mika gera para você;
- o Mika é instruído a tratar sinais de autolesão com acolhimento e a indicar serviços de apoio, mas
  **não é um serviço de emergência nem de saúde mental**: em crise, ligue para o número de emergência
  do seu país (Brasil: CVV 188 ou SAMU 192; EUA: 988; Europa: 112);
- se você publicar ou distribuir saída do Mika (texto, fala, código), indique que ela foi gerada por
  máquina — o modelo de voz padrão exige isso (seção 8).

## 4. Recursos opcionais e o que você autoriza

Alguns recursos acessam sua tela, voz, arquivos ou computador. Esta seção diz como cada um se
comporta nesta versão.

| Recurso                                              | Como é controlado                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Captura de tela e visão**                          | Desligada por padrão. Só captura quando **duas** chaves estão ligadas: "Visão" (`vision.enabled`) e "Captura de tela" (Privacidade, `privacy.screenCapture`). Janelas cujo título casa com a lista de bloqueio (por padrão: gerenciadores de senha, bancos, e-mail, mensageiros, navegação privada, carteiras cripto, saúde e autenticadores; você pode editá-la) não são capturadas, e a captura é recusada quando a janela em foco é a de Configurações ou do assistente do próprio Mika. Sem sonda da janela em foco (Linux/Wayland) a captura periódica nem roda, e o item "Pausar mic e tela" da bandeja suspende áudio e captura de uma vez. A imagem não é gravada em disco; ela é enviada ao provedor de visão que você configurou (local, se for o Ollama local). |
| **Área de transferência**                            | Desligada por padrão. Só é lida com "Registro da área de transferência" (Privacidade) ligado; imagens, só com a opção própria de visão.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **Microfone e voz**                                  | O reconhecimento de fala padrão roda no seu computador. O áudio só sai da máquina se você escolher um provedor remoto (Groq ou OpenAI) em Voz → Reconhecimento. O sistema operacional pede a permissão de microfone.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **Controle do computador** (mouse, teclado, janelas) | **Desligado por padrão** (perfis novos): você liga em Controle do computador. Ligado, **nenhuma ação é executada sem a sua aprovação em cada chamada**. No Windows, ela também recusa agir quando a janela em foco é do próprio Mika e em campos de senha (nos outros sistemas, só a sua aprovação protege, e a busca de elementos da tela não é suportada). Pode ser desligada de novo a qualquer momento.                                                                                                                                                                                                                                                                                                                                                                |
| **Execução de código**                               | O Mika executa JavaScript em um interpretador isolado, sem acesso a arquivos, rede ou ao sistema, e **só com a sua aprovação a cada execução**. Mesmo isolado, um script malicioso pode travar o aplicativo: aprove apenas o que você entende.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **Servidores MCP**                                   | Desligado por padrão. Servidores MCP são programas de terceiros que você instala e que rodam como processos no seu computador, com acesso próprio à rede e aos arquivos que o seu usuário alcança. **Cada chamada de ferramenta de um servidor MCP pede a sua aprovação.** Você responde por quais servidores adiciona.                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **Busca na web**                                     | Desligada por padrão (perfis novos): ao ligar em Navegador, o Mika pode pesquisar na web (Bing, DuckDuckGo ou Yahoo) quando a conversa pede, e o texto da busca e o seu IP chegam a esses serviços.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| **Consulta de pessoas na web**                       | Desligada por padrão (perfis novos): ao ligar em Catálogo de pessoas, o nome de figuras públicas, personagens, jogos e filmes recém-catalogados é enviado à Wikipédia e a buscadores. Pessoas com quem você tem relação nunca são pesquisadas.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **Steam**                                            | Opcional. Se você informar uma chave da Steam Web API e seu SteamID64, o Mika consulta a API da Steam (jogos e conquistas).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **Modo offline**                                     | Em Privacidade, bloqueia toda requisição externa (apenas `localhost` continua permitido) e desconecta servidores MCP.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |

Aprovar uma ação é decisão sua: você responde pelo que o Mika fizer a seu pedido ou com a sua
aprovação, inclusive por conteúdo de páginas web, arquivos ou telas que tentem instruir o Mika
("injeção de instruções"). Não aprove ações que você não pediu.

## 5. Serviços de terceiros e LLMs remotos

O Mika funciona com um modelo local (Ollama) por padrão. Você pode, por vontade própria, conectar
provedores remotos — Google (Gemini), OpenRouter, qualquer endpoint compatível com OpenAI, ElevenLabs,
Azure, Google TTS, Groq, OpenAI. Quando você faz isso:

- o conteúdo descrito em PRIVACY.md (mensagens, trechos de memória, perfil, imagens de tela, áudio)
  é enviado a esse provedor, que o trata sob os **termos e a política de privacidade dele**, não os
  nossos;
- as chaves e os custos são seus; não somos parte do seu contrato com o provedor;
- não controlamos a disponibilidade, a precisão, a moderação nem a retenção de dados do provedor, e
  não respondemos por eles (no OpenRouter, o Mika pede retenção zero e nenhuma coleta de dados em cada
  requisição de chat e de visão — um pedido que o OpenRouter aplica sob os termos dele; nos demais
  provedores não há esse pedido);
- você é responsável por cumprir as regras de uso aceitável de cada provedor.

## 6. Componentes e downloads de terceiros

O Mika inclui bibliotecas, runtimes, modelos 3D, animações e fontes de terceiros, e **baixa modelos
de voz e de IA (Hugging Face, GitHub) quando você usa o recurso correspondente**. Cada componente
segue a licença própria, listada em THIRD-PARTY-NOTICES.md e LICENSE-ASSETS.md. Essas licenças
prevalecem sobre este EULA no que diz respeito ao componente. Os modelos de Live2D de amostra não
acompanham o instalador; se você os instalar por conta própria, aceita a licença da Live2D.

## 7. Seus dados

O Mika guarda seus dados no seu computador. O que é enviado e para onde está em PRIVACY.md.

## 8. Modelos de terceiros com restrições de uso

O modelo de voz local padrão (Supertonic, da Supertone Inc.) é licenciado sob a **BigScience
OpenRAIL-M**, que nos obriga a repassar a você restrições de uso. Ao usar o Mika, você concorda em
**não usar o modelo de voz nem as saídas dele** para:

1. violar qualquer lei ou regulamento aplicável;
2. explorar ou prejudicar menores de idade, de qualquer forma;
3. gerar ou espalhar informação comprovadamente falsa com o fim de prejudicar outros;
4. gerar ou espalhar dados pessoais que possam ser usados para prejudicar alguém;
5. publicar conteúdo gerado por máquina sem deixar claro que ele é gerado por máquina;
6. difamar, desqualificar ou assediar pessoas;
7. se passar por outras pessoas sem o consentimento delas (inclusive _deepfakes_);
8. tomar decisões totalmente automatizadas que afetem direitos legais de alguém ou criem obrigação
   vinculante;
9. discriminar ou prejudicar pessoas ou grupos por comportamento social, características pessoais
   conhecidas ou previstas, ou categorias legalmente protegidas;
10. explorar vulnerabilidades de grupos (idade, condição social, física ou mental) para distorcer
    comportamento de forma que cause dano;
11. fornecer aconselhamento médico ou interpretar resultados médicos;
12. produzir informação para administração da justiça, polícia, imigração ou asilo, como prever que
    alguém cometerá fraude ou crime.

O texto integral e vinculante é a licença OpenRAIL-M em THIRD-PARTY-NOTICES.md. Outros modelos podem
trazer restrições próprias; veja o mesmo arquivo.

## 9. Idade mínima

O Mika é destinado a **maiores de 18 anos**. Ao aceitar este contrato você declara ter 18 anos ou
mais (a tela de aceite exige essa confirmação). Se você tem menos de 18 anos, não use o Mika.

## 10. Atualizações

Na Steam, as atualizações são entregues pelo cliente Steam. Em outras distribuições, o aplicativo
pode oferecer atualizações assinadas. Podemos alterar, suspender ou retirar recursos; funcionalidades
que dependem de serviços de terceiros podem deixar de funcionar sem aviso.

## 11. Propriedade intelectual

O Mika, a personagem, os prompts, o código, o ícone e os assets próprios pertencem a
[RAZÃO SOCIAL] e a seus licenciantes. Este EULA não concede nenhum direito além da licença da seção 2.
Marcas de terceiros citadas pertencem a seus titulares.

## 12. Sem garantia

Na máxima extensão permitida pela lei, o Mika é fornecido **"no estado em que se encontra"**, sem
garantia de qualquer tipo, expressa ou implícita, inclusive de adequação a um fim, de ausência de
erros ou de disponibilidade.

## 13. Limitação de responsabilidade

Na máxima extensão permitida pela lei, não respondemos por danos indiretos, incidentais, especiais ou
consequenciais, por perda de dados, lucros ou uso, nem por atos de provedores de terceiros escolhidos
por você. Nossa responsabilidade total limita-se ao valor que você pagou pelo Mika. **Nada neste
contrato limita direitos que a lei do seu país não permite limitar**, inclusive os do consumidor.

## 14. Rescisão

A licença termina automaticamente se você descumprir este contrato. Você pode encerrá-la a qualquer
momento desinstalando o Mika. As seções 3, 8, 11, 12, 13 e 15 sobrevivem ao término.

## 15. Lei aplicável e foro

Lei aplicável: **[LEI APLICÁVEL]**. Foro: **[FORO]**. Quando a lei assim determinar, o consumidor
pode propor ação no foro do seu domicílio.

## 16. Alterações deste contrato

Podemos atualizar este contrato. A versão vigente acompanha o aplicativo; mudanças relevantes serão
comunicadas na atualização ou na página do produto, e o uso continuado indica aceitação.

## 17. Contato

**[RAZÃO SOCIAL]** — [ENDEREÇO] — **[E-MAIL DE SUPORTE]**
