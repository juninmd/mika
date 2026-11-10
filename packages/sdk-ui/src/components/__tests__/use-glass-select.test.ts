import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from './fake-react'

vi.mock('react', () => import('./fake-react'))

const { useGlassSelect } = await import('../use-glass-select')
type GlassSelectProps = Parameters<typeof useGlassSelect>[0]

const OPTIONS = [
  { label: 'Alpha', value: 'a' },
  { label: 'Bravo', value: 'b' },
  { label: 'Charlie', value: 'c' },
]

function stubDom() {
  const listeners = new Map<string, Set<EventListener>>()
  function addListener(name: string, fn: EventListener) {
    if (!listeners.has(name)) listeners.set(name, new Set())
    listeners.get(name)?.add(fn)
  }
  function removeListener(name: string, fn: EventListener) {
    listeners.get(name)?.delete(fn)
  }
  vi.stubGlobal('window', {
    innerHeight: 800,
    innerWidth: 1200,
    addEventListener: vi.fn(addListener),
    removeEventListener: vi.fn(removeListener),
  })
  vi.stubGlobal('document', {
    addEventListener: vi.fn(addListener),
    removeEventListener: vi.fn(removeListener),
  })
  return {
    fireDocumentMousedown(target: object) {
      for (const fn of listeners.get('mousedown') ?? []) (fn as (e: object) => void)({ target })
    },
    hasListener(name: string) {
      return (listeners.get(name)?.size ?? 0) > 0
    },
  }
}

function setup(overrides: Partial<GlassSelectProps> = {}) {
  const props: GlassSelectProps = { label: 'Choice', options: OPTIONS, ...overrides }
  const hook = renderHook((p: GlassSelectProps) => useGlassSelect(p), props)
  return { hook, props }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('useGlassSelect — leitura de valor e busca', () => {
  it('lê o valor atual e a opção selecionada via config+configPath', () => {
    stubDom()
    const { hook } = setup({ config: { theme: 'b' }, configPath: 'theme' })
    expect(hook.current.currentValue).toBe('b')
    expect(hook.current.selectedOption).toEqual(OPTIONS[1])
  })

  it('não acha opção selecionada quando o valor não existe na lista', () => {
    stubDom()
    const { hook } = setup({ value: 'zzz' })
    expect(hook.current.selectedOption).toBeUndefined()
  })

  it('accentHue explícito vence o herdado do contexto', () => {
    stubDom()
    const { hook } = setup({ accentHue: 42 })
    expect(hook.current.accentHue).toBe(42)
  })

  it('filtra por texto quando searchable, sem diferenciar maiúsculas', () => {
    stubDom()
    const { hook } = setup({ searchable: true })
    hook.act(() => hook.current.setSearch('bra'))
    expect(hook.current.filteredOptions).toEqual([OPTIONS[1]])
  })

  it('ignora o texto de busca quando searchable é falso', () => {
    stubDom()
    const { hook } = setup({ searchable: false })
    hook.act(() => hook.current.setSearch('bra'))
    expect(hook.current.filteredOptions).toEqual(OPTIONS)
  })

  it('activeDescendant fica indefinido quando a busca não casa com nada', () => {
    stubDom()
    const { hook } = setup({ searchable: true })
    hook.act(() => hook.current.setSearch('zzz'))
    expect(hook.current.filteredOptions).toEqual([])
    expect(hook.current.activeDescendant).toBeUndefined()
  })
})

describe('useGlassSelect — abrir e fechar', () => {
  it('open() foca no índice do valor atual', () => {
    stubDom()
    const { hook } = setup({ value: 'c' })
    hook.act(() => hook.current.open())
    expect(hook.current.activeIndex).toBe(2)
    expect(hook.current.isOpen).toBe(true)
  })

  it('open() cai no índice 0 quando o valor atual não está na lista', () => {
    stubDom()
    const { hook } = setup({ value: 'missing' })
    hook.act(() => hook.current.open())
    expect(hook.current.activeIndex).toBe(0)
  })

  it('close(true) devolve o foco ao trigger; close(false) não chama focus', () => {
    stubDom()
    const { hook } = setup()
    const focus = vi.fn()
    hook.current.triggerRef.current = {
      focus,
      getBoundingClientRect: () => ({ top: 0, bottom: 0, left: 0, width: 0 }),
    } as unknown as HTMLButtonElement
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.close(true))
    expect(focus).toHaveBeenCalledOnce()
    expect(hook.current.isOpen).toBe(false)
    expect(hook.current.search).toBe('')

    hook.act(() => hook.current.open())
    hook.act(() => hook.current.close(false))
    expect(focus).toHaveBeenCalledOnce()
  })

  it('registra e remove os listeners de scroll/resize/mousedown só enquanto aberto', () => {
    const dom = stubDom()
    const { hook } = setup()
    hook.act(() => hook.current.open())
    expect(dom.hasListener('scroll')).toBe(true)
    expect(dom.hasListener('resize')).toBe(true)
    expect(dom.hasListener('mousedown')).toBe(true)

    hook.act(() => hook.current.close(false))
    expect(dom.hasListener('scroll')).toBe(false)
    expect(dom.hasListener('resize')).toBe(false)
    expect(dom.hasListener('mousedown')).toBe(false)
  })

  it('um mousedown fora do trigger e do painel fecha sem devolver o foco', () => {
    const dom = stubDom()
    const { hook } = setup()
    hook.act(() => hook.current.open())
    dom.fireDocumentMousedown({})
    hook.flush()
    expect(hook.current.isOpen).toBe(false)
  })
})

describe('useGlassSelect — navegação por teclado', () => {
  function fakeKeyEvent(key: string) {
    return { key, preventDefault: vi.fn() } as unknown as React.KeyboardEvent
  }

  it('ArrowDown/ArrowUp/Home/End movem activeIndex e chamam preventDefault', () => {
    stubDom()
    const { hook } = setup()
    hook.act(() => hook.current.open())

    const down = fakeKeyEvent('ArrowDown')
    hook.act(() => hook.current.handlePanelKeyDown(down))
    expect(hook.current.activeIndex).toBe(1)
    expect(down.preventDefault).toHaveBeenCalledOnce()

    const end = fakeKeyEvent('End')
    hook.act(() => hook.current.handlePanelKeyDown(end))
    expect(hook.current.activeIndex).toBe(2)

    const up = fakeKeyEvent('ArrowUp')
    hook.act(() => hook.current.handlePanelKeyDown(up))
    expect(hook.current.activeIndex).toBe(1)

    const home = fakeKeyEvent('Home')
    hook.act(() => hook.current.handlePanelKeyDown(home))
    expect(hook.current.activeIndex).toBe(0)
  })

  it('Escape fecha devolvendo o foco ao trigger', () => {
    stubDom()
    const { hook } = setup()
    const focus = vi.fn()
    hook.current.triggerRef.current = {
      focus,
      getBoundingClientRect: () => ({ top: 0, bottom: 0, left: 0, width: 0 }),
    } as unknown as HTMLButtonElement
    hook.act(() => hook.current.open())
    const esc = fakeKeyEvent('Escape')
    hook.act(() => hook.current.handlePanelKeyDown(esc))
    expect(hook.current.isOpen).toBe(false)
    expect(focus).toHaveBeenCalledOnce()
    expect(esc.preventDefault).toHaveBeenCalledOnce()
  })

  it('Tab fecha sem devolver o foco e sem preventDefault', () => {
    stubDom()
    const { hook } = setup()
    const focus = vi.fn()
    hook.current.triggerRef.current = {
      focus,
      getBoundingClientRect: () => ({ top: 0, bottom: 0, left: 0, width: 0 }),
    } as unknown as HTMLButtonElement
    hook.act(() => hook.current.open())
    const tab = fakeKeyEvent('Tab')
    hook.act(() => hook.current.handlePanelKeyDown(tab))
    expect(hook.current.isOpen).toBe(false)
    expect(focus).not.toHaveBeenCalled()
    expect(tab.preventDefault).not.toHaveBeenCalled()
  })

  it('Enter seleciona a opção ativa via onChange', () => {
    stubDom()
    const onChange = vi.fn()
    const { hook } = setup({ onChange })
    hook.act(() => hook.current.open())
    const enter = fakeKeyEvent('Enter')
    hook.act(() => hook.current.handlePanelKeyDown(enter))
    expect(onChange).toHaveBeenCalledWith('a')
    expect(enter.preventDefault).toHaveBeenCalledOnce()
  })

  it('Enter numa lista filtrada vazia não seleciona nem quebra', () => {
    stubDom()
    const onChange = vi.fn()
    const { hook } = setup({ onChange, searchable: true })
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.setSearch('zzz'))
    const enter = fakeKeyEvent('Enter')
    hook.act(() => hook.current.handlePanelKeyDown(enter))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('search encurtando a lista recolhe o activeIndex para dentro dela (setSearch do hook)', () => {
    stubDom()
    // Open com valor 'c' põe o cursor em 2; a busca 'bra' deixa só 1 opção.
    const { hook } = setup({ value: 'c', searchable: true })
    hook.act(() => hook.current.open())
    expect(hook.current.activeIndex).toBe(2)
    hook.act(() => hook.current.setSearch('bra'))
    expect(hook.current.filteredOptions).toEqual([OPTIONS[1]])
    expect(hook.current.activeIndex).toBe(0)
    expect(hook.current.activeDescendant).toBeDefined()
  })

  it('idem sem a muted: Enter recolhido seleciona a opção filtrada', () => {
    stubDom()
    const onChange = vi.fn()
    const { hook } = setup({ onChange, value: 'c', searchable: true })
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.setSearch('bra'))
    const enter = fakeKeyEvent('Enter')
    hook.act(() => hook.current.handlePanelKeyDown(enter))
    expect(onChange).toHaveBeenCalledWith('b')
  })

  it('search esvaziando a lista deixa o índice em 0 sem selecionar', () => {
    stubDom()
    const { hook } = setup({ value: 'c', searchable: true })
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.setSearch('zzz'))
    expect(hook.current.filteredOptions).toEqual([])
    expect(hook.current.activeIndex).toBe(0)
  })

  it('limpar a busca volta o índice para a lista completa sem sair do range', () => {
    stubDom()
    const { hook } = setup({ value: 'c', searchable: true })
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.setSearch('bra'))
    hook.act(() => hook.current.setSearch(''))
    expect(hook.current.activeIndex).toBe(0)
    expect(hook.current.filteredOptions).toEqual(OPTIONS)
  })

  it('uma tecla comum não move índice nem fecha', () => {
    stubDom()
    const { hook } = setup()
    hook.act(() => hook.current.open())
    const other = fakeKeyEvent('a')
    hook.act(() => hook.current.handlePanelKeyDown(other))
    expect(hook.current.activeIndex).toBe(0)
    expect(hook.current.isOpen).toBe(true)
    expect(other.preventDefault).not.toHaveBeenCalled()
  })
})

describe('useGlassSelect — selectOption e persistência', () => {
  it('sem onSave/configPath só chama onChange e fecha', () => {
    stubDom()
    const onChange = vi.fn()
    const { hook } = setup({ onChange, value: 'a' })
    hook.act(() => hook.current.open())
    hook.act(() => hook.current.selectOption('b'))
    expect(onChange).toHaveBeenCalledWith('b')
    expect(hook.current.isOpen).toBe(false)
    expect(hook.current.saveError).toBeNull()
  })

  it('onSave bem-sucedido mantém saveError nulo', async () => {
    stubDom()
    const onSave = vi.fn(async () => {})
    const { hook } = setup({ onSave, configPath: 'theme', config: { theme: 'a' } })
    hook.act(() => hook.current.selectOption('b'))
    await vi.advanceTimersByTimeAsync(0)
    hook.flush()
    expect(onSave).toHaveBeenCalledWith('theme', 'b')
    expect(hook.current.saveError).toBeNull()
  })

  it('onSave rejeitado publica a mensagem de erro em saveError', async () => {
    stubDom()
    const onSave = vi.fn(async () => {
      throw new Error('disk full')
    })
    const { hook } = setup({ onSave, configPath: 'theme', config: { theme: 'a' } })
    hook.act(() => hook.current.selectOption('b'))
    await vi.advanceTimersByTimeAsync(0)
    hook.flush()
    expect(hook.current.saveError).toBe('disk full')
  })

  it('onSave sem configPath não é chamado', () => {
    stubDom()
    const onSave = vi.fn(async () => {})
    const { hook } = setup({ onSave, value: 'a' })
    hook.act(() => hook.current.selectOption('b'))
    expect(onSave).not.toHaveBeenCalled()
  })
})
