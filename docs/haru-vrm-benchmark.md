# Haru VRM — comparação visual e orçamento HD

> **Superado em 15/09/2026.** A revisão "HD" descrita abaixo (roupa PBR, ACES, albedo facial e íris
> procedurais, calota de cabelo própria) foi revertida por deixar a Haru com aspecto de boneco de
> plástico. O estado atual — MToon cel com contorno, pinturas VRoid recoloridas, ~317 mil triângulos
> — está em [HARU.md](../apps/desktop/src/renderer/public/vrm/mika/HARU.md). As referências e os
> limites abaixo continuam válidos; as medidas e a seção "Implementado" não.

Consulta: 14/09/2026. Para quem está escolhendo o acabamento da Haru: o que mudou em relação ao
asset anterior e às cinco referências indicadas? Esta é uma comparação de arte e estrutura de
assets, não um benchmark de FPS, GPU ou qualidade equivalente a um console.

## Referências indicadas

As cinco imagens de apresentação foram abertas e comparadas. Os números abaixo são os publicados
nas páginas; os pacotes Unity não foram importados nem medidos localmente. As páginas não expõem
uma data editorial inequívoca. Diferenças de pose, câmera e iluminação impedem uma pontuação visual
objetiva. As observações estéticas são interpretação, não propriedades técnicas comprovadas.

| Referência                                           | Triângulos publicados | Observação visual aproveitada                                                              |
| ---------------------------------------------------- | --------------------: | ------------------------------------------------------------------------------------------ |
| [Laffy](https://vrcmods.com/item?id=11166)           |                93.547 | Lapelas, contraste entre camisa e blazer, olhos definidos. Não copiar proporções infantis. |
| [Akula](https://vrcmods.com/item?id=10874)           |               100.638 | Mechas separadas, íris com contraste, sombra suave na pele.                                |
| [Mob Chan](https://vrcmods.com/item?id=10897)        |       cerca de 51.800 | Agrupamento das mechas e reflexo localizado no cabelo; apenas referência de acabamento.    |
| [Raiden Shogun](https://vrcmods.com/item?id=10078)   |       cerca de 25.500 | Silhueta legível, camadas de roupa e peças com movimento independente.                     |
| [Dark Knight Haku](https://vrcmods.com/item?id=6582) |      cerca de 148.600 | Separação entre tecido, detalhes metálicos e cabelo; não reproduzir o figurino.            |

Laffy e Akula informam uso de lilToon/Poiyomi; as outras páginas citam Poiyomi ou Dynamic Bones.
Esses pacotes não foram incorporados. A Haru mantém o pipeline glTF/Three.js e SpringBone VRM.
Os arquivos de terceiros não foram redistribuídos; as imagens ficaram apenas na pasta local ignorada
`.homologation/haru-art/references/`. A existência de um download público não comprova licença para
redistribuir os assets, especialmente nas conversões de personagens de jogos.

## Medidas do asset Haru

Contagem dos índices das primitivas do GLB, sem passes adicionais de sombra/outline:

| Medida                              | Antes desta revisão |  Haru HD |
| ----------------------------------- | ------------------: | -------: |
| Triângulos totais                   |              81.212 |  255.540 |
| Corpo visível                       |               5.864 |   93.824 |
| Cabelo principal                    |              16.600 |   66.400 |
| Blazer                              |              12.288 |   24.576 |
| Saia                                |               8.192 |   16.384 |
| Cadeias SpringBone próprias da Haru |                   3 |       13 |
| Tamanho aproximado do VRM           |            18,9 MiB | 22,6 MiB |

A densidade total é aproximadamente 3,15 vezes a anterior e superior à das cinco referências.
Isso não significa qualidade superior: a comparação mostra justamente que distribuição da malha,
texturas e direção de arte importam tanto quanto a contagem. O orçamento estrutural do asset passa
a 28 MiB. Não há comprovação de que esse custo seja adequado a toda GPU usada com a Mika.

## Implementado

- Tesselação curva em duas etapas para corpo/mãos/pernas e uma para cabelo e calçados. Os novos
  vértices interpolam UVs, normais e pesos dos ossos; as bordas abertas conservam o contorno.
- Malhas geradas indexadas, evitando repetir todos os vértices de cada triângulo. A topologia das
  57 formas faciais permanece preservada, acompanhando os ajustes de proporção.
- Proporções menos extremas entre busto e cintura; transições contínuas e mangas encaixadas com
  raiz afilada. Dobras geométricas discretas, lapelas curvas e lenço sem anéis de torus.
- Roupa com materiais PBR: tecido fosco, seda mais lisa e metais com brilho próprio. Normal map
  procedural 1024² para microtrama, cor do tecido 512². Pele e cabelo mantêm MToon para leitura
  anime; reflexo matcap suave nas mechas e coloração de bochechas por vértice.
- Iluminação exclusiva da Haru com luz principal, preenchimento, recorte, hemisférica, sombras
  2048² e tone mapping ACES. Canvas transparente e iluminação das demais skins preservados.
- Oito cadeias na saia, duas nas mechas junto ao rosto, uma nos cachos e duas no lenço. Colisores
  de cabeça, tronco e coxas; rigidez, gravidade e amortecimento no próprio VRM. A atualização usa
  o relógio existente do avatar, sem um segundo loop.

SpringBone é movimento secundário por ossos, **não** simulação completa de tecido com colisão
triângulo a triângulo. A saia continua ajustada; sua parte superior permanece presa ao quadril.

## Evidência e limites

O asset foi regenerado e renderizado isoladamente no Chromium, sem iniciar Electron. Foram
inspecionadas vistas de frente, perfil, três quartos e rosto, além de poses de aceno, comemoração
e reflexão. As capturas atuais ficam em `.homologation/haru-art/`; as vistas anteriores estão em
`before-hd/`. A captura informa os nomes das imagens efetivamente produzidas, evitando confundir
arquivos antigos com poses da revisão atual.

Uma sequência curta de aceno foi gravada em `.homologation/haru-art/haru-motion-reel.webm`.
A gravação usa o mesmo modelo e adaptador, com atualização de física entre quadros. Ela ilustra
movimento, mas não comprova estabilidade de todas as colisões nem desempenho no computador do usuário.

Não foram executados testes, gates, typecheck ou build, conforme pedido. Os testes de estrutura
foram atualizados, mas não executados. O módulo `avatar-render` foi reaberto: o certificado antigo
não é evidência desta revisão. Não foram medidos FPS, VRAM ou estabilidade prolongada da física.

Ainda há distância para um personagem AAA esculpido e texturizado manualmente. O rosto é derivado
da base pixiv, a roupa é procedural e não há tecido físico completo, scattering de pele ou cabelo
por fios. Portanto, esta revisão não deve ser apresentada como “gráficos de PS5” comprovados.

## Fundamentação técnica

- [VRM SpringBone 1.0](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_springBone-1.0/README.md): inércia, rigidez, gravidade e colisores esféricos/cápsulas; especificação marcada como completa.
- [VRM MToon 1.0](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_materials_mtoon-1.0/README.md): controle da transição de luz/sombra e suporte a normal maps.
- [VRoid Studio](https://vroid.com/en/studio): camadas de roupa e movimento configurável por mecha; inspiração de fluxo, sem importar modelos.
- [Guilty Gear Strive — PlayStation](https://www.playstation.com/en-us/games/guilty-gear-strive/): referência de direção anime 2D/3D, não de orçamento de polígonos.
- [Stellar Blade — entrevista, 08/04/2024](https://blog.playstation.com/2024/04/08/stellar-blade-interview-creating-stylish-sci-fi-action-in-a-post-apocalyptic-world/): contexto de produção para console; não fornece um orçamento técnico diretamente comparável à Haru.
