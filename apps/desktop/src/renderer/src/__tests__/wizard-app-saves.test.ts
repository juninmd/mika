import { ConfigKey } from '@mika/sdk/contracts'
import { describe, expect, it, vi } from 'vitest'

const saveSetting = vi.fn().mockResolvedValue({ success: true })

vi.mock('../lib/shell-api', () => ({
  getElectronApi: () => ({ runtime: { saveSetting } }),
}))

const {
  saveFreshInstallMemoryDefault,
  saveFreshInstallMicDefault,
  saveLlmSelection,
  saveMemoryChoice,
  savePrivacyChoices,
  savePrivacyStep,
  readWizardRestore,
  saveTtsCompletion,
  saveSttCompletion,
  finishWizardSetup,
  WIZARD_STEPS,
} = await import('../wizard-app-saves')

describe('saveLlmSelection', () => {
  it('persiste o modelo na chave específica do provedor escolhido', async () => {
    await saveLlmSelection('openrouter', 'openai/gpt-oss-20b:free', '', undefined)

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LlmProvider, 'openrouter')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LlmModel, 'openai/gpt-oss-20b:free')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LlmOpenrouterModel, 'openai/gpt-oss-20b:free')
  })

  // O Unsloth tem chave e modelo PRÓPRIOS: gravar a chave `sk-unsloth-…` em `openaiCompatibleApiKey`
  // a mandaria para o servidor errado.
  it('grava a chave e o modelo do Unsloth nos campos dele, nunca nos do openaiCompatible', async () => {
    saveSetting.mockClear()
    await saveLlmSelection('unsloth', 'gemma-4-26B-A4B-it-GGUF', ' sk-unsloth-abc ', ConfigKey.UnslothApiKey)

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LlmProvider, 'unsloth')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LlmUnslothModel, 'gemma-4-26B-A4B-it-GGUF')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.UnslothApiKey, 'sk-unsloth-abc')
    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.LlmOpenaiCompatibleModel, expect.anything())
    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.OpenAiCompatibleApiKey, expect.anything())
  })
})

describe('savePrivacyStep', () => {
  // O aceite do EULA/política viaja com o passo de privacidade: o "Continuar" que grava mic/tela
  // é o mesmo que fecha o aceite, e uma falha em qualquer um dos dois tem de aparecer no mesmo erro.
  it('grava as escolhas de privacidade e o aceite da versão exibida', async () => {
    saveSetting.mockClear()
    const results = await savePrivacyStep({ microphone: false, screen: true }, true, 'v9')

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.PrivacyScreenCapture, true)
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LegalAcceptedVersion, 'v9')
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.LegalAcceptedAt, expect.stringMatching(/^\d{4}-\d\d-\d\dT/))
    expect(results.every((result) => result.success)).toBe(true)
  })

  it('devolve a falha do aceite em vez de escondê-la atrás do sucesso das escolhas', async () => {
    saveSetting.mockImplementation(async (key: string) => ({ success: key !== ConfigKey.LegalAcceptedVersion }))
    const results = await savePrivacyStep({ microphone: false, screen: false }, true, 'v9')
    saveSetting.mockResolvedValue({ success: true })

    expect(results.some((result) => !result.success)).toBe(true)
  })
})

describe('savePrivacyChoices', () => {
  it('aplica os defaults conservadores de memória/clipboard numa instalação nova', async () => {
    await savePrivacyChoices({ microphone: true, screen: false }, false)

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.VoiceSttEnabled, true)
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.PrivacyClipboardLogging, false)
    expect(saveSetting).toHaveBeenCalledWith('vision.readClipboardImages', false)
    expect(saveSetting).toHaveBeenCalledWith('vision.visualDiaryEnabled', false)
  })

  // Embedding (memória) vem ANTES deste passo agora: gravar `memory.enabled=false` aqui apagaria a
  // escolha que o usuário acabou de fazer.
  it('não toca na memória — ela é decidida no passo de embedding', async () => {
    saveSetting.mockClear()
    await savePrivacyChoices({ microphone: false, screen: false }, false)

    expect(saveSetting).not.toHaveBeenCalledWith('memory.enabled', expect.anything())
  })

  it('NÃO reaplica os defaults conservadores quando o wizard é refeito num app já configurado', async () => {
    saveSetting.mockClear()
    await savePrivacyChoices({ microphone: true, screen: true }, true)

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.VoiceSttEnabled, true)
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.VisionEnabled, true)
    expect(saveSetting).not.toHaveBeenCalledWith('memory.enabled', false)
    expect(saveSetting).not.toHaveBeenCalledWith('vision.readClipboardImages', false)
    expect(saveSetting).not.toHaveBeenCalledWith('vision.visualDiaryEnabled', false)
    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.PrivacyClipboardLogging, false)
  })
})

describe('saveFreshInstallMemoryDefault', () => {
  // O LLM vem antes da memória: retomar o wizard depois dele leria o default do schema (`true`)
  // como se fosse escolha — o `false` explícito do 1º passo é o que o toggle lê de volta.
  it('grava memória desligada explicitamente', async () => {
    saveSetting.mockClear()
    await saveFreshInstallMemoryDefault()

    expect(saveSetting).toHaveBeenCalledWith('memory.enabled', false)
  })
})

describe('saveFreshInstallMicDefault', () => {
  // `voice.sttEnabled` tem default `true` no schema e a config volta com os defaults mesclados: sem
  // um `false` explícito, o passo "ficam desligados até você ativar" abria com o microfone LIGADO.
  it('grava a escuta desligada explicitamente', async () => {
    saveSetting.mockClear()
    await saveFreshInstallMicDefault()

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.VoiceSttEnabled, false)
  })
})

describe('saveMemoryChoice', () => {
  it('grava exatamente a escolha do passo de embedding', async () => {
    saveSetting.mockClear()
    await saveMemoryChoice(true)

    expect(saveSetting).toHaveBeenCalledWith('memory.enabled', true)
  })
})

describe('saveTtsCompletion', () => {
  // A voz não é mais o último passo (Avatar/Identidade vêm depois): marcar o wizard como
  // concluído aqui deixaria os dois passos seguintes sem terem sido percorridos.
  it('NÃO marca o wizard como concluído — isso é responsabilidade de finishWizardSetup', async () => {
    saveSetting.mockClear()
    await saveTtsCompletion('supertonic')

    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.SetupWizardCompleted, true)
  })
})

describe('saveSttCompletion', () => {
  // A escuta liga/desliga no passo de privacidade (toggle de microfone): o passo de STT escolhe
  // QUEM escuta (`voice.asrProvider`), nunca o senso liga literal (mesma separação do TTS, que
  // escolhe provedor, e o toggle "voz" da voz mora na config).
  it('grava o provedor de escuta com o valor canônico do schema (voice.asrProvider)', async () => {
    saveSetting.mockClear()
    await saveSttCompletion('groq-whisper')

    expect(saveSetting).toHaveBeenCalledWith('voice.asrProvider', 'groq-whisper')
    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.VoiceSttEnabled, expect.anything())
  })

  it('o recomendado local (nemotron) grava sem alias', async () => {
    saveSetting.mockClear()
    await saveSttCompletion('nemotron')

    expect(saveSetting).toHaveBeenCalledWith('voice.asrProvider', 'nemotron')
  })

  it('NÃO marca o wizard como concluído — isso é responsabilidade de finishWizardSetup', async () => {
    saveSetting.mockClear()
    await saveSttCompletion('nemotron')

    expect(saveSetting).not.toHaveBeenCalledWith(ConfigKey.SetupWizardCompleted, true)
  })
})

describe('finishWizardSetup', () => {
  it('marca o wizard concluído e reseta o passo salvo para o início', async () => {
    saveSetting.mockClear()
    await finishWizardSetup()

    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.SetupWizardCompleted, true)
    expect(saveSetting).toHaveBeenCalledWith(ConfigKey.SetupWizardCurrentStep, 'language')
  })
})

describe('WIZARD_STEPS', () => {
  // Skin antes do cérebro: a primeira fala gerada pelo LLM já pode comentar o próprio visual.
  // Escuta logo depois da voz: os dois sentidos de áudio chegam juntos — quem fala e quem ouve.
  it('segue a ordem dos sentidos: corpo, cérebro, voz, escuta, memória, olhos/ouvidos', () => {
    expect(WIZARD_STEPS).toEqual([
      'language',
      'avatar',
      'llm',
      'tts',
      'stt',
      'embedding',
      'privacy',
      'identity',
      'done',
    ])
  })
})

describe('readWizardRestore', () => {
  it('restaura mic/tela do que já está salvo, em vez de sempre começar desligado', () => {
    const restored = readWizardRestore({
      voice: { sttEnabled: true },
      privacy: { screenCapture: true },
    })

    expect(restored.privacy).toEqual({ microphone: true, screen: true })
  })

  it('restaura o provedor de escuta salvo (stt) para pré-selecionar o passo', () => {
    expect(readWizardRestore({ voice: { asrProvider: 'groq-whisper' } }).stt).toBe('groq-whisper')
  })

  it('o alias legado local-whisper volta como electron (mesmo card que as Configurações acendem)', () => {
    expect(readWizardRestore({ voice: { asrProvider: 'local-whisper' } }).stt).toBe('electron')
  })

  it('provedor fora do leque do wizard fica null (mantém o que as Configurações têm)', () => {
    expect(readWizardRestore({ voice: { asrProvider: 'kokoro-stt' } }).stt).toBeNull()
    expect(readWizardRestore({ voice: {} }).stt).toBeNull()
  })

  // `privacy` era o 2º passo da ordem antiga: retomá-lo na ordem nova pularia skin, cérebro, voz e
  // memória, e a pessoa terminaria o onboarding com memória ligada sem ter escolhido.
  it('recomeça do início um `privacy` salvo sem LLM (ordem antiga)', () => {
    expect(readWizardRestore({ setup: { wizardCurrentStep: 'privacy' } }).step).toBeUndefined()
  })

  it('retoma `privacy` normalmente quando o cérebro já foi escolhido', () => {
    const restored = readWizardRestore({ setup: { wizardCurrentStep: 'privacy' }, llm: { provider: 'ollama' } })
    expect(restored.step).toBe('privacy')
  })

  it('só considera o Unsloth configurado quando a chave dele está salva', () => {
    const unsloth = { setup: { wizardCurrentStep: 'privacy' }, llm: { provider: 'unsloth' } }
    expect(readWizardRestore(unsloth).llmConfigured).toBe(false)
    expect(readWizardRestore({ ...unsloth, apiKeys: { unslothApiKey: 'sk-unsloth-abc' } }).llmConfigured).toBe(true)
    expect(readWizardRestore({ ...unsloth, apiKeys: { openaiCompatibleApiKey: 'outra' } }).llmConfigured).toBe(false)
  })

  it('mantém mic/tela desligados quando a config não tem os campos (instalação nova)', () => {
    const restored = readWizardRestore({})

    expect(restored.privacy).toEqual({ microphone: false, screen: false })
  })
})
