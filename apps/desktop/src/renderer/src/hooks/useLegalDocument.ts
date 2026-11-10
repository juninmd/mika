import type { LegalDocumentId } from '@mika/sdk/contracts'
import { useEffect, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

export type LegalDocState =
  | { status: 'idle' | 'loading' | 'error' }
  | { status: 'ready'; text: string; version: string }

/**
 * Lê o documento EMPACOTADO (`legal:getDocument`); `null` = nada aberto. A `version` é a do aceite.
 * Mudar `attempt` relê o documento (o "Tentar de novo" de quem viu `error`).
 */
export function useLegalDocument(doc: LegalDocumentId | null, attempt = 0): LegalDocState {
  const [state, setState] = useState<LegalDocState>({ status: 'idle' })
  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` só existe para re-disparar a leitura ("Tentar de novo")
  useEffect(() => {
    if (doc === null) {
      setState({ status: 'idle' })
      return
    }
    let cancelled = false
    setState({ status: 'loading' })
    getElectronApi()
      .runtime.getLegalDocument(doc)
      .then((result) => {
        if (!cancelled) setState({ status: 'ready', text: result.text, version: result.version })
      })
      .catch((error) => {
        rendererLog.error('legal document load failed', { doc, error })
        if (!cancelled) setState({ status: 'error' })
      })
    return () => {
      cancelled = true
    }
  }, [doc, attempt])
  return state
}
