import type { LlmEndpoint } from './chat.service'
import { saveEndpoint } from './endpoint.storage'

interface EndpointFormProps {
  value: LlmEndpoint
  onChange: (value: LlmEndpoint) => void
}

export function EndpointForm({ value, onChange }: EndpointFormProps) {
  const update = (patch: Partial<LlmEndpoint>) => {
    const next = { ...value, ...patch }
    saveEndpoint(localStorage, next)
    onChange(next)
  }
  return (
    <section className="settings" aria-label="Provedor de LLM">
      <label>
        URL (compatível com OpenAI)
        <input value={value.baseUrl} onChange={(e) => update({ baseUrl: e.target.value })} />
      </label>
      <label>
        Modelo
        <input value={value.model} onChange={(e) => update({ model: e.target.value })} />
      </label>
      <label>
        Chave de API (opcional, só nesta sessão)
        <input type="password" value={value.apiKey} onChange={(e) => update({ apiKey: e.target.value })} />
      </label>
    </section>
  )
}
