import { beforeEach, describe, expect, it } from 'vitest'
import { useChatStore } from '../useChatStore'

const steps = () => useChatStore.getState().turnSteps
const store = () => useChatStore.getState()

/** Checklist de passos do turno: um passo por chamada, fechado na ordem certa, nunca carregando o turno anterior. */
describe('turnSteps', () => {
  beforeEach(() => useChatStore.setState({ turnSteps: [] }))

  it('uma chamada de ferramenta vira um passo ativo', () => {
    store().beginStep('vision_read', 'Lendo a tela')
    expect(steps()).toEqual([{ id: 'vision_read', label: 'Lendo a tela', status: 'active' }])
  })

  it('o resultado fecha o passo', () => {
    store().beginStep('steam', 'Steam')
    store().finishStep('steam', true)
    expect(steps()[0].status).toBe('done')
  })

  it('falha da ferramenta aparece como erro, não como concluído', () => {
    store().beginStep('steam', 'Steam')
    store().finishStep('steam', false)
    expect(steps()[0].status).toBe('error')
  })

  it('a mesma ferramenta duas vezes no turno vira dois passos, fechados na ordem certa', () => {
    store().beginStep('steam', 'Steam')
    store().finishStep('steam', true)
    store().beginStep('steam', 'Steam')

    expect(steps().map((s) => s.status)).toEqual(['done', 'active'])

    store().finishStep('steam', false)
    expect(steps().map((s) => s.status)).toEqual(['done', 'error'])
  })

  it('resultado sem passo aberto não inventa passo nem quebra', () => {
    store().finishStep('fantasma', true)
    expect(steps()).toEqual([])
  })

  it('chamada repetida sem resultado no meio não duplica o passo ativo', () => {
    store().beginStep('vision_read', 'Lendo a tela')
    store().beginStep('vision_read', 'Lendo a tela')
    expect(steps()).toHaveLength(1)
  })

  it('o turno novo começa vazio — a checklist não carrega o turno anterior', () => {
    store().beginStep('steam', 'Steam')
    store().finishStep('steam', true)
    store().resetTurnSteps()
    expect(steps()).toEqual([])
  })

  it('turno com muitas ferramentas mantém só os passos recentes', () => {
    for (let i = 0; i < 10; i++) {
      store().beginStep(`tool_${i}`, `Ferramenta ${i}`)
      store().finishStep(`tool_${i}`, true)
    }
    expect(steps()).toHaveLength(6)
    expect(steps()[5].id).toBe('tool_9')
  })
})
