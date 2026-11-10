import { t } from '@mika/sdk/i18n'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MATCH_EFFECT_IDS } from '../../lib/match-effect'
import {
  buildHelpText,
  CLEAR_CONFIRM_WINDOW_MS,
  type ClearGuard,
  handleSlashCommand,
  isFatalSpeechError,
} from '../shell-commands'

function base() {
  return {
    messages: [],
    clearGuard: { armedAt: null } as ClearGuard,
    doClearHistory: vi.fn(),
    setBubbleVisible: vi.fn(),
    addNotification: vi.fn(),
    setAppStatus: vi.fn(),
  }
}

describe('handleSlashCommand — efeitos por comando direto', () => {
  it.each(MATCH_EFFECT_IDS)('/%s dispara o toggle do efeito correspondente', (id) => {
    const deps = { ...base(), onEffectCommand: vi.fn() }
    const consumed = handleSlashCommand({ ...deps, text: `/${id}` })

    expect(consumed).toBe(true)
    expect(deps.onEffectCommand).toHaveBeenCalledWith(id)
    expect(deps.setAppStatus).toHaveBeenCalledWith('idle')
  })

  it('o comando consumido não vai para o modelo nem imprime balão', () => {
    const deps = { ...base(), onEffectCommand: vi.fn() }
    handleSlashCommand({ ...deps, text: '/sakura' })

    expect(deps.setBubbleVisible).not.toHaveBeenCalled()
  })

  it('sinônimo da tool (ex.: /zen, /hack) NÃO é consumido aqui — segue para o modelo e a tool', () => {
    const deps = { ...base(), onEffectCommand: vi.fn() }
    expect(handleSlashCommand({ ...deps, text: '/zen' })).toBe(false)
    expect(handleSlashCommand({ ...deps, text: '/hack' })).toBe(false)
    expect(deps.onEffectCommand).not.toHaveBeenCalled()
  })

  it('sem onEffectCommand o comando continua consumido — não vira fala para o modelo', () => {
    const deps = base()
    expect(handleSlashCommand({ ...deps, text: '/matrix' })).toBe(true)
  })

  it('comando com argumentos extras ainda casa o comando (/matrix agora)', () => {
    const deps = { ...base(), onEffectCommand: vi.fn() }
    expect(handleSlashCommand({ ...deps, text: '/matrix agora' })).toBe(true)
    expect(deps.onEffectCommand).toHaveBeenCalledWith('matrix')
  })

  it('/help anuncia os efeitos disponíveis', () => {
    const deps = base()
    handleSlashCommand({ ...deps, text: '/help' })

    const message = deps.addNotification.mock.calls[0]?.[1] as string
    expect(message).toContain('/matrix')
    expect(message).toContain('/fireworks')
  })
})

// window.confirm não renderiza no WebView do Tauri: a confirmação é repetir o comando dentro da janela.
describe('handleSlashCommand — /clear e /limpar exigem confirmação em dois passos', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('o primeiro /clear só arma e avisa — não apaga nada', () => {
    const deps = base()

    const consumed = handleSlashCommand({ ...deps, text: '/clear', now: 1_000 })

    expect(consumed).toBe(true)
    expect(deps.doClearHistory).not.toHaveBeenCalled()
    expect(deps.clearGuard.armedAt).toBe(1_000)
    expect(deps.addNotification).toHaveBeenCalledWith('warning', t('shell.commands.clearArmed'), expect.any(String))
    expect(deps.setAppStatus).toHaveBeenCalledWith('idle')
  })

  it('não depende de window.confirm (que no Tauri não aparece e devolveria false)', () => {
    const confirm = vi.fn().mockReturnValue(false)
    vi.stubGlobal('window', { confirm })
    const deps = base()

    handleSlashCommand({ ...deps, text: '/clear', now: 0 })
    handleSlashCommand({ ...deps, text: '/clear', now: 1 })

    expect(confirm).not.toHaveBeenCalled()
    expect(deps.doClearHistory).toHaveBeenCalledTimes(1)
  })

  it('repetir dentro da janela apaga e desarma (/clear e /limpar misturados)', () => {
    const deps = base()

    handleSlashCommand({ ...deps, text: '/clear', now: 0 })
    handleSlashCommand({ ...deps, text: '/limpar', now: CLEAR_CONFIRM_WINDOW_MS })

    expect(deps.doClearHistory).toHaveBeenCalledTimes(1)
    expect(deps.setBubbleVisible).toHaveBeenCalledWith(false)
    expect(deps.clearGuard.armedAt).toBeNull()
    expect(deps.addNotification).toHaveBeenLastCalledWith(
      'success',
      t('shell.commands.cleared'),
      t('shell.commands.title'),
    )
  })

  it('repetir depois da janela só rearma — um /clear velho não vale como confirmação', () => {
    const deps = base()

    handleSlashCommand({ ...deps, text: '/clear', now: 0 })
    handleSlashCommand({ ...deps, text: '/clear', now: CLEAR_CONFIRM_WINDOW_MS + 1 })

    expect(deps.doClearHistory).not.toHaveBeenCalled()
    expect(deps.clearGuard.armedAt).toBe(CLEAR_CONFIRM_WINDOW_MS + 1)
  })

  it('depois de apagar, o próximo /clear volta a exigir os dois passos', () => {
    const deps = base()

    for (const now of [0, 1, 2]) handleSlashCommand({ ...deps, text: '/clear', now })

    expect(deps.doClearHistory).toHaveBeenCalledTimes(1)
    expect(deps.clearGuard.armedAt).toBe(2)
  })
})

describe('handleSlashCommand — textos saem do dicionário do locale', () => {
  it('/help lista cada comando uma vez, sem os sinônimos, com a descrição traduzida', () => {
    const deps = base()
    handleSlashCommand({ ...deps, text: '/help' })

    const [, message, title] = deps.addNotification.mock.calls[0] as [string, string, string]
    expect(title).toBe(t('shell.composer.commands.title'))
    expect(message).toContain(`/clear — ${t('shell.composer.commands.clear')}`)
    expect(message).not.toContain('/limpar')
    expect(message.split('\n')).toHaveLength(5)
  })

  it('/stats interpola as contagens sem sobrar placeholder', () => {
    const deps = {
      ...base(),
      messages: [
        { role: 'user', content: 'a' },
        { role: 'assistant', content: 'b' },
        { role: 'assistant', content: 'c' },
      ] as never,
    }
    handleSlashCommand({ ...deps, text: '/stats' })

    const [, message, title] = deps.addNotification.mock.calls[0] as [string, string, string]
    expect(title).toBe(t('shell.commands.statsTitle'))
    expect(message).not.toContain('{{')
    expect(message).toMatch(/^3 .*1.*2$/)
  })

  it('buildHelpText acompanha SLASH_COMMAND_DEFS (mesma lista do autocomplete)', () => {
    expect(buildHelpText()).toContain(`/export — ${t('shell.composer.commands.export')}`)
  })
})

describe('isFatalSpeechError — match por token inteiro, não substring solta', () => {
  it('códigos de erro exatos continuam fatais', () => {
    expect(isFatalSpeechError('not-allowed')).toBe(true)
    expect(isFatalSpeechError('NotAllowedError')).toBe(true)
    expect(isFatalSpeechError('audio-capture')).toBe(true)
  })

  it('mensagem livre contendo "network" no meio da frase NÃO é mais fatal (falha transitória)', () => {
    expect(isFatalSpeechError('network')).toBe(false)
    expect(isFatalSpeechError('Failed to fetch model: network error, retrying')).toBe(false)
    expect(isFatalSpeechError('fetch failed due to network timeout while downloading whisper model')).toBe(false)
  })

  it('substring solta dentro de outra palavra não deve casar como token fatal', () => {
    // "NotFoundError" é fatal, mas não deve casar dentro de uma palavra maior sem fronteira.
    expect(isFatalSpeechError('SomeNotFoundErrorWrapperClass')).toBe(false)
    expect(isFatalSpeechError('Model asset NotFoundError: whisper-base.onnx')).toBe(true)
  })
})
