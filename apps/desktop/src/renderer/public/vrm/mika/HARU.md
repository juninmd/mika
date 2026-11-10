# Haru — Celestial Greeter VRM 3

Variante 3D fiel à Haru Live2D (`public/live2d/haru`), cuja textura serviu de referência de paleta e
desenho, sem copiar pixels: chanel lavanda-acinzentado com cacho em espiral preso por elástico
listrado, olhos azuis, blazer grafite com debrum claro no barrado, lapelas e punhos, V claro, lenço
listrado em diagonal, saia lápis, meia-calça escura translúcida no joelho e sapatilhas com laço.

Direção de arte (revisão de 15/09/2026, substitui a "HD" de 14/09, que usava PBR e ACES e deixava a
figura com aspecto de boneco de plástico):

- Todo material visível é MToon cel (sombra dura, `shadingToony` 0,93) com contorno bordô em
  coordenadas de mundo. `haru-asset.test.ts` falha se alguma peça voltar a ser PBR sem traço.
- Rosto, íris e cabelo partem das pinturas VRoid originais, recoloridas pela luminância
  (`recolorPng`): pupila, reflexos e gradientes sobrevivem. O rosto fica quase sempre iluminado.
- O chanel dobra as mechas longas da base até o queixo, preservando as pontas afiladas.
- Proporção de heroína anime adulta: ~7,4 cabeças, pernas a 55% da altura (a revisão anterior as
  encurtava para 49% sem encurtar os braços). Pernas esculpidas em `haru-anatomy.mjs`: patela, recuo
  atrás do joelho, panturrilha com pico interno mais alto, tornozelo e tendão finos.
- A meia-calça é assada nos UVs das pernas da base, com dilatação para não abrir costura.
- Estúdio de luz próprio, sem tone mapping; as outras skins mantêm a iluminação padrão.
- Densidade: ~317 mil triângulos (base: 36 mil). Corpo e sapatos em dois níveis de tesselação curva;
  rosto e boca em um nível, com as 57 expressões interpoladas e regravadas como morphs esparsos.
- O gerador compacta o binário no fim (`haru-compact.mjs`): só vai ao arquivo o que o glTF referencia.

Física: 8 cadeias na saia, 1 no cacho e 2 no lenço, com colisores de cabeça, tronco e coxas.
É movimento secundário por ossos, não simulação de tecido.

Regenerar: `node scripts/generators/generate-haru-vrm.mjs`, na raiz do repositório.
O gerador preserva a base `mika.vrm` e os créditos/licença embutidos e escreve `haru.vrm`.

Selecione **Haru (VRM)** na aba de skins. Configurações antigas com `mika.vrm#look=haru` também
carregam esta variante. As demais skins continuam usando a base.

O catálogo do VRM oferece 12 coreografias: `wave`, `shrug`, `cheer`, `shy`, `thinking`, `laugh`,
`bow`, `stretch`, `presenting`, `peace`, `heart` e `victory`. Cabeça, tronco, punhos e dedos participam
dos gestos; a preparação, a pausa da pose e o retorno têm curvas próprias. Interromper um gesto
inicia uma transição de 240 ms a partir da pose atual. Negação e piscadela continuam disponíveis.
São coreografias do adaptador Mika, não clips VRMA exportados no arquivo. O movimento secundário
do cabelo e do lenço faz parte do VRM e pode ser interpretado por outros leitores compatíveis.

Vida em runtime (16/09/2026). Vale para qualquer VRM VRoid, não só para a Haru:

- Cabeça e pescoço acompanham o cursor, com 55% do ângulo e limites de 40°, 10° para cima e 22°
  para baixo (`vrm-gaze.ts`). O olho faz sacadas curtas em volta do alvo e chega ao giro máximo com
  25° de olhar, em vez de 90°.
- A piscada pausa enquanto happy ou relaxed já fecham os olhos, e às vezes é dupla (`vrm-blink.ts`).
- Os visemas abrem rápido e fecham devagar (`vrm-mouth.ts`). O som da fala sai 50 ms depois do
  analisador, para a boca não chegar atrasada.
- Uma brisa entra na gravidade das molas, na ordem de grandeza da gravidade autoral (`vrm-wind.ts`).
- Íris e brilho nunca recebem sombra. O rosto só sombreia na borda (`shadingShift` ≥ 0,5). A sombra
  da pele tende ao coral. O contorno engrossa na silhueta (`vrm-materials.ts`).
- Um passe de tela aplica nitidez limitada, brilho difuso só nas partes claras, tons divididos e
  saturação, preservando o alfa. Fica ligado apenas na qualidade `full` (`vrm-postfx.ts`).

Técnicas estudadas no [AnimeVRM](https://github.com/uemegu/AnimeVRM), que não tem licença:
nenhum código ou asset foi copiado.

Conferência isolada, sem iniciar o app: `pnpm -s exec tsx scripts/generators/render-haru-preview.mts`.
Para apenas produzir imagens, sem as verificações do estúdio, acrescente `--art-only`;
essa modalidade escreve em `.homologation/haru-art/`, fora das pastas que a suíte E2E limpa.
Acrescente `--video` para registrar os gestos em WebM, ou `--thumbnail` para atualizar a miniatura;
após atualizar a miniatura, rode novamente o gerador para embuti-la no VRM. As capturas e o relatório
ficam em `apps/desktop/e2e/screenshots/haru-studio/`. Requer o Chromium do Playwright instalado.
Os testes `haru-asset.test.ts` e `vrm-body-rig.test.ts` verificam o arquivo distribuído e as curvas.
O E2E do app permanece em `apps/desktop/e2e/specs/haru-vrm.spec.ts`; a renderização isolada não o substitui.

Referências consultadas em 13/09/2026, aplicadas como princípios e sem copiar assets:

- [AnimSchool — Acting Through Anticipation, 19/07/2024](https://blog.animschool.edu/2024/07/19/acting-through-anticipation/): preparação e intenção antes do gesto.
- [VRoid — Hair Materials](https://vroid.pixiv.help/hc/en-us/articles/900005965706-How-to-edit-hair-color-and-texture-material): base, reflexos e contraste de mechas; página sem data editorial explícita.
- [VRM — SpringBone 1.0](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_springBone-1.0/README.md): inércia, rigidez e amortecimento para cabelo e roupa.
- [VRM Animation](https://vrm.dev/en/vrma/): distinção entre animação portátil VRMA e a coreografia executada pelo adaptador.
