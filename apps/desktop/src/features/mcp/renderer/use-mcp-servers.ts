import type { McpServerSummary, McpServersListResponse } from '@mika/sdk/contracts'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useRef, useState } from 'react'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { toggledSpec } from './mcp-server-form'

export type McpListState =
  | { status: 'loading' }
  | { status: 'failed'; error: string }
  | { status: 'ready'; data: McpServersListResponse }

export type McpActionError = { name: string; message: string }

const runtime = () => getElectronApi().runtime

/**
 * Lista viva de `mcp.json`: relê no `onMcpServersChanged` (salvar, remover, rodada de conexão,
 * MCP ligado/desligado) e quando `mcp.enabled` muda na tela. Ligar/desligar é otimista e desfaz
 * se o backend recusar.
 */
export function useMcpServers(mcpEnabled: boolean) {
  const [state, setState] = useState<McpListState>({ status: 'loading' })
  const [busy, setBusy] = useState<Record<string, true>>({})
  const [actionError, setActionError] = useState<McpActionError | null>(null)
  const mounted = useRef(true)

  const reload = useCallback(async () => {
    try {
      const data = await runtime().getMcpServers()
      if (mounted.current) setState({ status: 'ready', data })
    } catch (error) {
      if (mounted.current)
        setState((prev) => (prev.status === 'ready' ? prev : { status: 'failed', error: toErrorMessage(error) }))
    }
  }, [])

  useEffect(() => {
    mounted.current = true
    const unsubscribe = runtime().onMcpServersChanged(() => void reload())
    return () => {
      mounted.current = false
      unsubscribe()
    }
  }, [reload])

  useEffect(() => {
    void mcpEnabled
    void reload()
  }, [reload, mcpEnabled])

  const patchServer = useCallback((name: string, patch: Partial<McpServerSummary>) => {
    setState((prev) =>
      prev.status !== 'ready'
        ? prev
        : {
            ...prev,
            data: { ...prev.data, servers: prev.data.servers.map((s) => (s.name === name ? { ...s, ...patch } : s)) },
          },
    )
  }, [])

  const withBusy = useCallback(
    async (name: string, action: () => Promise<{ ok: boolean; error?: string }>, rollback?: () => void) => {
      setBusy((b) => ({ ...b, [name]: true }))
      setActionError(null)
      try {
        const result = await action()
        if (!result.ok) throw new Error(result.error)
      } catch (error) {
        rollback?.()
        if (mounted.current) setActionError({ name, message: toErrorMessage(error) })
      } finally {
        if (mounted.current) {
          setBusy(({ [name]: _, ...rest }) => rest)
          void reload()
        }
      }
    },
    [reload],
  )

  const toggle = useCallback(
    (server: McpServerSummary, enabled: boolean) => {
      patchServer(server.name, { enabled })
      return withBusy(
        server.name,
        () =>
          runtime().saveMcpServer({ name: server.name, originalName: server.name, spec: toggledSpec(server, enabled) }),
        () => patchServer(server.name, { enabled: server.enabled }),
      )
    },
    [patchServer, withBusy],
  )

  const remove = useCallback((name: string) => withBusy(name, () => runtime().removeMcpServer({ name })), [withBusy])

  const openConfigDir = useCallback(async () => {
    setActionError(null)
    try {
      const result = await runtime().openMcpConfigDir()
      if (!result.ok) setActionError({ name: '', message: result.error })
    } catch (error) {
      setActionError({ name: '', message: toErrorMessage(error) })
    }
  }, [])

  return { state, busy, actionError, dismissError: () => setActionError(null), reload, toggle, remove, openConfigDir }
}
