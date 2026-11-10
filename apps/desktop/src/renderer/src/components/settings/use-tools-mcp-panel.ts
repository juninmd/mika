import { useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi, type ToolCatalogItem } from '../../lib/shell-api'
import type { LabeledOption } from './settings-utils'
import { type ExecEntry, extractToolError, groupByPlugin, MCP_PLUGIN_ID, PLUGIN_LABEL_KEYS } from './tools-mcp-catalog'

export function useToolsMcpPanel() {
  const { t } = useI18n()
  const [tools, setTools] = useState<ToolCatalogItem[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [mcpTools, setMcpTools] = useState<ToolCatalogItem[]>([])
  const [search, setSearch] = useState('')
  const [recent, setRecent] = useState<ExecEntry[]>([])
  const uidRef = useRef(0)
  const [reloadToken, setReloadToken] = useState(0)

  const pluginLabels = useMemo<LabeledOption[]>(
    () => Object.entries(PLUGIN_LABEL_KEYS).map(([value, key]) => ({ value, label: t(key) })),
    [t],
  )

  useEffect(() => {
    let mounted = true
    void reloadToken
    setLoading(true)
    setLoadError(false)
    getElectronApi()
      .runtime.getTools()
      .then((result) => {
        if (mounted) setTools(result.tools)
      })
      .catch(() => {
        if (mounted) setLoadError(true)
      })
      .finally(() => {
        if (mounted) setLoading(false)
      })
    return () => {
      mounted = false
    }
  }, [reloadToken])

  useEffect(() => {
    let mounted = true
    void reloadToken
    getElectronApi()
      .runtime.getToolsByPlugin(MCP_PLUGIN_ID)
      .then((result) => {
        if (mounted) setMcpTools(result.tools)
      })
      .catch(() => {
        if (mounted) setMcpTools([])
      })
    return () => {
      mounted = false
    }
  }, [reloadToken])

  useEffect(() => {
    const unsub = getElectronApi().runtime.onToolResult((payload) => {
      uidRef.current += 1
      const success = payload.success !== false
      const entry: ExecEntry = {
        uid: uidRef.current,
        name: payload.name,
        success,
        durationMs: payload.durationMs,
        error: success ? undefined : extractToolError(payload.result),
      }
      setRecent((prev) => [entry, ...prev].slice(0, 8))
    })
    return () => unsub()
  }, [])

  const deferredSearch = useDeferredValue(search)
  const filtered = useMemo(() => {
    const term = deferredSearch.trim().toLowerCase()
    if (!term) return tools
    return tools.filter(
      (tool) => tool.name.toLowerCase().includes(term) || (tool.description ?? '').toLowerCase().includes(term),
    )
  }, [tools, deferredSearch])

  const groups = useMemo(() => groupByPlugin(filtered), [filtered])

  return {
    t,
    tools,
    loading,
    loadError,
    mcpTools,
    search,
    setSearch,
    recent,
    setReloadToken,
    pluginLabels,
    deferredSearch,
    filtered,
    groups,
  }
}
