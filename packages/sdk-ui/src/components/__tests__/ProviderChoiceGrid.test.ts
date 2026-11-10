import { describe, expect, it } from 'vitest'
import { nextProviderForKey, type ProviderChoice } from '../ProviderChoiceGrid'

const PROVIDERS: ProviderChoice[] = [
  { id: 'ollama', label: 'Ollama', desc: 'local' },
  { id: 'openai', label: 'OpenAI', desc: 'remoto' },
  { id: 'anthropic', label: 'Anthropic', desc: 'remoto' },
]

describe('ProviderChoiceGrid — nextProviderForKey', () => {
  it('avança para o próximo provedor com ArrowDown', () => {
    expect(nextProviderForKey('ArrowDown', PROVIDERS, 'ollama')?.id).toBe('openai')
  })

  it('volta para o provedor anterior com ArrowUp', () => {
    expect(nextProviderForKey('ArrowUp', PROVIDERS, 'openai')?.id).toBe('ollama')
  })

  it('dá a volta do último provedor para o primeiro', () => {
    expect(nextProviderForKey('ArrowRight', PROVIDERS, 'anthropic')?.id).toBe('ollama')
  })

  it('dá a volta do primeiro provedor para o último', () => {
    expect(nextProviderForKey('ArrowLeft', PROVIDERS, 'ollama')?.id).toBe('anthropic')
  })

  it('ignora teclas irrelevantes (ex: Tab)', () => {
    expect(nextProviderForKey('Tab', PROVIDERS, 'ollama')).toBeNull()
  })

  it('retorna null quando o provedor ativo não está na lista', () => {
    expect(nextProviderForKey('ArrowDown', PROVIDERS, 'unknown-provider')).toBeNull()
  })

  it('retorna null com um único provedor (não trava, apenas não avança)', () => {
    const single: ProviderChoice[] = [{ id: 'ollama', label: 'Ollama', desc: 'local' }]
    expect(nextProviderForKey('ArrowDown', single, 'ollama')?.id).toBe('ollama')
  })
})
