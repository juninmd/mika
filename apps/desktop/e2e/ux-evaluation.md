# Avaliação de UX / UI — Auditoria e Melhorias Proativas (Ciclo 2)

Arquivo temporário de homologação visual e refinamento de experiência de usuário.

---

## 1. Testes de Interação do Usuário Realizados

1. **Envio de Mensagens pelo Chat (`shell-composer` & `shell-chat-sent`):**
   - Abertura com `Ctrl+Space`.
   - Digitação de mensagem pelo usuário.
   - Envio via `Enter`: feedback imediato, entrada no fluxo de resposta, balão de fala da Mika posicionado sem sobrepor o avatar nem sensores.
2. **Menu de Contexto por Clique com Botão Direito (`shell-context-menu`):**
   - Clique com botão direito na área da Mika abre menu pop-up com ações essenciais (Conversar, Puxar assunto, Histórico, Microfone, Fixar no topo, Configurações, Limpar histórico).
   - Navegação por teclado (`ArrowDown`, `ArrowUp`, `Escape`) validada.
3. **Arrastar Janela (`shell-after-drag`):**
   - Área superior com classe `electron-drag` (`h-10`) permite arrastar e posicionar o avatar na tela livremente.
4. **Frequência de "Sentindo o Clima" (Tool `mood`):**
   - **Diagnóstico:** A tool `mood` estava com descrição genérica (`Mika's mood and affinity...`), levando a LLM a chamar a tool desnecessariamente para "inspecionar o clima" em conversas casuais.
   - **Ação:** O humor atual já é injetado como contexto (`[HUMOR ATUAL]`). A descrição da tool foi atualizada para proibir chamadas de inspeção casual e reservar a tool estritamente para consultas explícitas de afinidade ou marcos emocionais significativos.

---

## 2. Ajustes Visuais e Ergonômicos Implementados

1. **ChatComposer & Quick Actions:**
   - Elevação do container (`bottom-20`) para distanciar da barra de dock inferior.
   - Botões de ações rápidas (`Tela`, `Jogo`, `Quem é?`, `Reage`, `Surpresa`) compactados (`px-2.5 py-1 text-[10.5px]`) para caberem em uma única linha fluida.
2. **Posicionamento de Sensores e Balões de Fala:**
   - Sensores de telemetria reposicionados para `top-3 right-3`.
   - `SystemPulse` reposicionado para `top-3 left-3` e tamanho reduzido para `h-6 w-6` ("Mika Core").
   - `SpeechBubble` ajustado para `top-48`, garantindo que o rosto do avatar e os sensores fiquem 100% visíveis mesmo quando a Mika está falando.
3. **Menu de Contexto:**
   - Menus com `z-index: var(--layer-dropdown)`, backdrop-blur e clamp à viewport, fechando ao clicar fora ou apertar `Esc`.

---

## 3. Contenção Estrita na Janela (Zero Cortes em Menus, Popups e Tooltips)

1. **MikaContextMenu (`apps/desktop/src/renderer/src/components/MikaContextMenu.tsx`):**
   - Implementado cálculo preditivo que inclui separadores e medição dinâmica precisa via `useLayoutEffect` com `getBoundingClientRect()`.
   - Adicionado `max-h-[calc(100vh-16px)]` e `overflow-y-auto` tanto no wrapper quanto na lista interna.
   - Mesmo que o usuário clique próximo à borda inferior ou lateral, o menu reposiciona automaticamente respeitando `VIEWPORT_MARGIN = 8px`.
2. **Tooltips Flutuantes (`apps/desktop/src/renderer/src/components/ui/Tooltip.tsx`):**
   - Adicionado detecção de limites da viewport (`useLayoutEffect`):
     - Inversão vertical inteligente: quando o elemento está no topo da tela (sensores do HUD, Mika Core), o tooltip inverte automaticamente para `top-full mt-2` em vez de vazar para cima.
     - Compensação horizontal (`xShift`): quando o elemento está colado nas bordas laterais, o balão desliza internamente para manter 8px de respiro, preservando a ponta apontando para o gatilho.
3. **Menu Radial / Roda Neural (`apps/desktop/src/renderer/src/components/NeuralWheel.tsx`):**
   - Margem segura de borda (`BUTTON_MARGIN`) aumentada de 40 para 56px, prevenindo que botões radiais e labels ultrapassem a tela em janelas compactas.
4. **Diálogos de Sistema e Notificações:**
   - `ToolApprovalDialog.tsx`: largura e altura agora usam `w-[min(24rem,calc(100vw-1.5rem))]` e `max-h-[calc(100vh-1.5rem)] overflow-y-auto`.
   - `NotificationToast.tsx`: container de toasts com `w-[min(18rem,calc(100vw-1.5rem))]` e `max-h-[calc(100vh-12rem)] overflow-y-auto`.
   - `TelemetryOptIn.tsx`: card contido com `p-4` no container e `max-h-[calc(100vh-2rem)] overflow-y-auto`.

---

## 4. Validação dos Gates

- `pnpm lint`: Biome clean (0 erros).
- `pnpm gates`: 24/24 gates aprovados.
- `pnpm typecheck`: 0 erros nos 7 pacotes.
- `pnpm test:unit`: 119 suítes e 788 testes passando.
- `pnpm build`: Build de produção Turbo + Vite concluído com sucesso.
- `pnpm test (ux-audit)`: 3 specs E2E aprovadas; asserção explícita de contenção visual `menuBox <= viewport` e screenshots homologados.
