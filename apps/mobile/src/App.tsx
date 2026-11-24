import type { VTuberExpression } from '@mika/sdk/contracts'
import { type FormEvent, useCallback, useState } from 'react'
import { AvatarView } from './AvatarView'
import { type ChatTurn, type LlmEndpoint, sendChat } from './chat.service'
import { EndpointForm } from './EndpointForm'
import { loadEndpoint } from './endpoint.storage'

// Lip-sync without TTS: the mouth moves for a time proportional to the reply length.
const speakingMs = (text: string) => Math.min(8000, 1200 + text.length * 45)

export function App() {
  const [endpoint, setEndpoint] = useState<LlmEndpoint>(() => loadEndpoint(localStorage))
  const [showSettings, setShowSettings] = useState(false)
  const [turns, setTurns] = useState<ChatTurn[]>([])
  const [draft, setDraft] = useState('')
  const [busy, setBusy] = useState(false)
  const [speaking, setSpeaking] = useState(false)
  const [expression, setExpression] = useState<VTuberExpression>('neutral')
  const [status, setStatus] = useState('')
  const onStatus = useCallback((s: string) => setStatus(s), [])

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const text = draft.trim()
    if (!text || busy) return
    const next: ChatTurn[] = [...turns, { id: turns.length, role: 'user', content: text }]
    setTurns(next)
    setDraft('')
    setBusy(true)
    setExpression('thinking')
    try {
      const reply = await sendChat(endpoint, next)
      setTurns([...next, { id: next.length, role: 'assistant', content: reply }])
      setExpression('happy')
      setSpeaking(true)
      setTimeout(() => setSpeaking(false), speakingMs(reply))
    } catch (error) {
      setExpression('confused')
      setStatus(`Erro no LLM: ${String(error)}`)
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="app">
      <AvatarView expression={expression} speaking={speaking} onStatus={onStatus} />
      <header className="topbar">
        <span className="title">MIKA · ANDROID</span>
        <button type="button" className="ghost" onClick={() => setShowSettings((v) => !v)}>
          {showSettings ? 'Fechar' : 'Provedor'}
        </button>
      </header>
      {showSettings && <EndpointForm value={endpoint} onChange={setEndpoint} />}
      <section className="chat" aria-live="polite">
        {turns.slice(-4).map((turn) => (
          <p key={turn.id} className={`bubble ${turn.role}`}>
            {turn.content}
          </p>
        ))}
        {busy && <p className="bubble assistant">Pensando...</p>}
        {status && <p className="status">{status}</p>}
      </section>
      <form className="composer" onSubmit={submit}>
        <input
          aria-label="Mensagem para a Mika"
          placeholder="Fale com a Mika..."
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <button type="submit" disabled={busy || !draft.trim()}>
          Enviar
        </button>
      </form>
    </main>
  )
}
