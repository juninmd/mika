import { ConfigKey } from '@mika/sdk/contracts'
import { useCallback, useEffect, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { type LegalDocState, useLegalDocument } from './useLegalDocument'

type LegalGateInput = {
  docStatus: LegalDocState['status']
  version: string
  acceptedVersion: string | null
  ageConfirmed: boolean | null
}

/**
 * Pura: o que o gate mostra. `needed` = pedir o aceite; `failed` = o texto não carregou, e sem texto
 * não há como saber a versão nem aceitar — o app NÃO abre (falhar aberto deixaria usar sem aceite).
 */
export function legalGateStatus({ docStatus, version, acceptedVersion, ageConfirmed }: LegalGateInput) {
  const known = version !== '' && acceptedVersion !== null
  const stale = known && acceptedVersion !== version
  // Aceite de antes do gate de idade (versão em dia, sem "18 anos ou mais") também reabre o diálogo.
  return {
    needed: stale || (known && ageConfirmed === false),
    updated: stale && acceptedVersion !== '',
    failed: docStatus === 'error',
  }
}

/**
 * O aceite está em dia? Versão do texto E `ageConfirmed`. `acceptedVersion` é `null` até a config ser lida e `version` é `''` até o
 * Rust responder — nenhuma das duas incógnitas abre o diálogo (piscar o aviso a quem já aceitou é
 * pior que esperar um tick). Escuta `config-changed` porque a janela principal já está montada
 * (escondida) enquanto o wizard grava o aceite: sem isso o diálogo ficaria preso por cima do avatar.
 * Documento ilegível ≠ incógnita: vira `failed` (com `retry`), não libera o app.
 */
export function useLegalGate() {
  const [attempt, setAttempt] = useState(0)
  const doc = useLegalDocument('eula', attempt)
  const version = doc.status === 'ready' ? doc.version : ''
  const [acceptedVersion, setAcceptedVersion] = useState<string | null>(null)
  const [ageConfirmed, setAgeConfirmed] = useState<boolean | null>(null)

  useEffect(() => {
    const runtime = getElectronApi().runtime
    let cancelled = false
    const stop = runtime.onConfigChanged((change) => {
      if (change.path === ConfigKey.LegalAcceptedVersion && typeof change.value === 'string') {
        setAcceptedVersion(change.value)
      }
      if (change.path === ConfigKey.LegalAgeConfirmed && typeof change.value === 'boolean') {
        setAgeConfirmed(change.value)
      }
    })
    runtime
      .getSettingsConfig()
      .then(({ config }) => {
        const legal = config.legal as Record<string, unknown> | undefined
        if (cancelled) return
        setAcceptedVersion((current) => current ?? String(legal?.acceptedVersion ?? ''))
        setAgeConfirmed((current) => current ?? legal?.ageConfirmed === true)
      })
      .catch((error) => rendererLog.error('legal gate: config unavailable', { error }))
    return () => {
      cancelled = true
      stop()
    }
  }, [])

  const retry = useCallback(() => setAttempt((n) => n + 1), [])
  return { ...legalGateStatus({ docStatus: doc.status, version, acceptedVersion, ageConfirmed }), version, retry }
}
