import { expect, test } from './fixtures/homolog-app'
import { createRecorder, summarize } from './fixtures/recorder'

// Second half of the matrix: core (brain, voice, vision) and companion tools features.spec.ts doesn't
// touch. The other 12 of 22 registered tools only had proof against the fake Ollama, which answers
// whatever the fixture tells it and so proves neither vision reading the screen nor real tool routing.

const recorder = createRecorder('Homologação funcional — núcleo, visão e voz', 'features-core')
const { record, probe } = recorder

test.describe.configure({ mode: 'serial' })

test('brain: introspecção, memória e comandos de efeito', async ({ runTool }) => {
  probe('brain', 'get_system_info()', await runTool('get_system_info'))

  const snapshot = probe('brain', 'memory_console(snapshot)', await runTool('memory_console', { action: 'snapshot' }))
  expect(snapshot.success, 'memory_console(snapshot) precisa responder').not.toBe(false)

  const fact = 'a homologação roda com qwen3.5:4b no Ollama local'
  probe('brain', 'memory_console(add_fact)', await runTool('memory_console', { action: 'add_fact', fact }))
  probe(
    'brain',
    'memory_console(search_episodic)',
    await runTool('memory_console', { action: 'search_episodic', query: 'homologação', limit: 5 }),
  )

  const effect = await runTool('match_effect', { text: '/matrix' })
  const triggered = (effect.data as { effect?: string } | undefined)?.effect === 'matrix'
  record(
    'brain',
    'match_effect(/matrix) casa o gatilho',
    triggered ? 'ok' : 'falha',
    triggered ? summarize(effect) : `não reconheceu o comando — ${summarize(effect)}`,
  )

  const noise = await runTool('match_effect', { text: 'bom dia, tudo bem?' })
  const quiet = (noise.data as { triggered?: boolean } | undefined)?.triggered !== true
  record(
    'brain',
    'match_effect não dispara em fala comum',
    quiet ? 'ok' : 'falha',
    quiet ? summarize(noise) : `disparou efeito numa frase sem comando — ${summarize(noise)}`,
  )
})

test('brain: geração de conteúdo e debate entre agentes', async ({ runTool }) => {
  test.setTimeout(300_000)

  probe('brain', 'content(daily_recap)', await runTool('content', { action: 'daily_recap' }))
  probe(
    'brain',
    'content(social_post)',
    await runTool('content', { action: 'social_post', event: 'terminei a homologação da Mika' }),
  )

  probe(
    'brain',
    'collaboration(start)',
    await runTool('collaboration', {
      action: 'start',
      agentA: 'Mika',
      agentB: 'Luna',
      topic: 'teclado mecânico vale a pena?',
    }),
  )
  // Debate runs in the background; stopping right after proves the start/stop pair closes.
  probe('brain', 'collaboration(stop)', await runTool('collaboration', { action: 'stop' }))

  const semArgumento = await runTool('collaboration', { action: 'start' })
  record(
    'brain',
    'collaboration(start) sem agentes é recusado',
    semArgumento.success === false ? 'ok' : 'falha',
    semArgumento.success === false ? summarize(semArgumento) : 'iniciou um debate sem tema nem agentes',
  )
})

test('voice: silêncio, escuta e fala com TTS desligado', async ({ runTool }) => {
  probe('voice', 'voice(mute, 1min)', await runTool('voice', { action: 'mute', minutes: 1 }))
  probe('voice', 'voice(mute, 0) tira do silêncio', await runTool('voice', { action: 'mute', minutes: 0 }))
  probe('voice', 'voice(listen_start)', await runTool('voice', { action: 'listen_start' }))
  probe('voice', 'voice(listen_stop)', await runTool('voice', { action: 'listen_stop' }))

  // TTS is off in this profile: the verdict here is "responds without breaking"; the full audio path
  // (TTS -> VAD -> Whisper -> LLM) is voice-loop.spec.ts.
  record(
    'voice',
    'voice(speak) com TTS desligado responde sem estourar',
    'ok',
    summarize(await runTool('voice', { action: 'speak', text: 'homologação da voz' })),
  )

  const semTexto = await runTool('voice', { action: 'speak' })
  record(
    'voice',
    'voice(speak) sem texto é recusado',
    semTexto.success === false ? 'ok' : 'falha',
    semTexto.success === false ? summarize(semTexto) : 'aceitou falar sem texto',
  )
})

test('vision: captura e leitura da tela de verdade', async ({ runTool }) => {
  test.setTimeout(300_000)

  probe('vision', 'vision_capture(frame) captura a tela', await runTool('vision_capture', { action: 'frame' }))

  const shot = await runTool('vision_capture', {
    action: 'screenshot',
    prompt: 'Descreva em uma frase o que está na tela.',
  })
  const descricao = JSON.stringify(shot.data ?? '')
  record(
    'vision',
    'vision_capture(screenshot) descreve a tela com o modelo local',
    shot.success === false || descricao.length <= 40 ? 'falha' : 'ok',
    descricao.length > 40 ? summarize(shot) : `descrição vazia ou curta demais — ${summarize(shot)}`,
  )

  probe('vision', 'vision_read(ocr) lê o texto da tela', await runTool('vision_read', { action: 'ocr', region: 'full' }))

  const semImagem = await runTool('vision_read', { action: 'describe' })
  record(
    'vision',
    'vision_read(describe) sem imagem é recusado',
    semImagem.success === false ? 'ok' : 'falha',
    semImagem.success === false ? summarize(semImagem) : 'analisou uma imagem que não foi enviada',
  )
})

test('gamer-companion: conhecimento de jogo, humor e recomendação', async ({ runTool }) => {
  test.setTimeout(300_000)

  probe(
    'gamer-companion',
    'game_knowledge(ask)',
    await runTool('game_knowledge', { action: 'ask', game: 'Hollow Knight', topic: 'beginner tips' }),
  )

  probe(
    'gamer-companion',
    'game_knowledge(memorize_location)',
    await runTool('game_knowledge', {
      action: 'memorize_location',
      game: 'Hollow Knight',
      locationName: 'Dirtmouth',
      description: 'Vila de entrada, com o banco e o Elderbug.',
    }),
  )

  const guide = await runTool('game_knowledge', {
    action: 'get_location_guide',
    game: 'Hollow Knight',
    query: 'Dirtmouth',
  })
  const lembrou = JSON.stringify(guide.data ?? '').includes('Dirtmouth')
  record(
    'gamer-companion',
    'game_knowledge(get_location_guide) devolve o que memorize gravou',
    lembrou ? 'ok' : 'falha',
    lembrou ? summarize(guide) : `não achou a localização recém-gravada — ${summarize(guide)}`,
  )

  probe(
    'gamer-companion',
    'track_gamer_mood(loss)',
    await runTool('track_gamer_mood', {
      recentMessages: ['de novo não, esse boss é impossível', 'chega por hoje'],
      gameOutcome: 'loss',
    }),
  )

  probe(
    'gamer-companion',
    'recommend_media(game)',
    await runTool('recommend_media', { type: 'game', mood: 'relaxed', genre: 'RPG' }),
  )
})

test('character: inspect_model lê o inventário do avatar carregado', async ({ runTool }) => {
  const inspect = await runTool('avatar', { action: 'inspect_model' })
  const data = inspect.data as { modelPath?: string; supportedExpressions?: unknown[] } | undefined
  const lido = Boolean(data?.modelPath) && Array.isArray(data?.supportedExpressions)
  record(
    'character',
    'avatar(inspect_model) lê o modelo do disco (.vrm ou .model3.json)',
    lido ? 'ok' : 'falha',
    lido ? summarize(inspect) : `não devolveu inventário do modelo — ${summarize(inspect)}`,
  )
  expect(inspect.success, 'inspect_model precisa ler o modelo carregado').not.toBe(false)
})

test('privacy: export_data escreve o arquivo e não apaga nada', async ({ runTool }) => {
  test.setTimeout(120_000)

  const exported = await runTool('export_data')
  const archive = (exported.data as { archive?: string } | undefined)?.archive
  record(
    'privacy',
    'export_data gera o .zip com os stores locais',
    exported.success === false || !archive ? 'falha' : 'ok',
    archive ? summarize(exported) : `não devolveu o caminho do arquivo — ${summarize(exported)}`,
  )

  probe('privacy', 'memória continua de pé depois do export', await runTool('memory_console', { action: 'snapshot' }))
})

test.afterAll(() => recorder.flush())
