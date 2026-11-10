import { t } from '@mika/sdk/i18n'
import { useCallback, useEffect, useState } from 'react'
import { rendererLog } from '../../src/lib/renderer-logger'
import { getElectronApi } from '../../src/lib/shell-api'
import { useUIStore } from '../../src/store/useUIStore'
import { downloadSocialGraph } from './memory-export'
import type { EpisodicMemory } from './memory-panel-cards'
import type { SnapshotData } from './memory-panel-sections'

type ToolResult<T = unknown> = { success: boolean; data?: T; error?: string }

async function callTool<T = unknown>(name: string, args: Record<string, unknown>): Promise<ToolResult<T>> {
  return getElectronApi().runtime.executeTool<T>(name, args)
}

export function useMemoryConsole() {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [snapshot, setSnapshot] = useState<SnapshotData | null>(null)
  const [episodic, setEpisodic] = useState<EpisodicMemory[]>([])
  const [searchQuery, setSearchQuery] = useState('')
  const [searchedTerm, setSearchedTerm] = useState<string | null>(null)
  const [newFactText, setNewFactText] = useState('')
  const [factFilter, setFactFilter] = useState('')
  const [newName, setNewName] = useState('')
  const [busyFact, setBusyFact] = useState(false)
  const [busyName, setBusyName] = useState(false)
  const [busyExport, setBusyExport] = useState(false)

  const runTool = async (name: string, args: Record<string, unknown>): Promise<boolean> => {
    try {
      const result = await callTool(name, args)
      if (!result.success) {
        if (result.error) rendererLog.error(`tool ${name} failed`, result.error)
        setError(t('settings.memoryConsole.error.run', { name }))
        return false
      }
      setError(null)
      return true
    } catch {
      setError(t('settings.memoryConsole.error.run', { name }))
      return false
    }
  }

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await callTool<SnapshotData>('memory_console', { action: 'snapshot' })
      if (!result.success) {
        if (result.error) rendererLog.error('memory_console snapshot failed', result.error)
        setError(t('settings.memoryConsole.error.load'))
        return
      }
      const data = result.data
      setSnapshot(data ?? null)
      setEpisodic(data?.episodic ?? [])
      setSearchedTerm(null)
      setNewName(data?.graph?.profile?.preferredName ?? '')
    } catch {
      setError(t('settings.memoryConsole.error.load'))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const searchEpisodic = async (query?: string) => {
    if (loading) return
    setLoading(true)
    setError(null)
    try {
      const result = await callTool<{ memories?: EpisodicMemory[] }>('memory_console', {
        action: 'search_episodic',
        query,
        limit: 100,
      })
      if (!result.success) {
        if (result.error) rendererLog.error('memory_console search_episodic failed', result.error)
        setError(t('settings.memoryConsole.error.search'))
        return
      }
      setEpisodic(result.data?.memories ?? [])
      setSearchedTerm(query ?? null)
    } catch {
      setError(t('settings.memoryConsole.error.search'))
    } finally {
      setLoading(false)
    }
  }

  const deleteEpisodic = async (id: string) => {
    const ok = await runTool('memory_console', { action: 'delete_episodic', id })
    if (ok) setEpisodic((prev) => prev.filter((m) => m.id !== id))
  }

  const addFact = async () => {
    const text = newFactText.trim()
    if (!text || busyFact) return
    setBusyFact(true)
    const ok = await runTool('memory_console', { action: 'add_fact', fact: text })
    if (ok) {
      setNewFactText('')
      await load()
    }
    setBusyFact(false)
  }

  const deleteFact = async (id: string) => {
    const ok = await runTool('memory_console', { action: 'delete_fact', id })
    if (ok) await load()
  }

  const saveName = async () => {
    const name = newName.trim()
    if (!name || busyName) return
    setBusyName(true)
    const ok = await runTool('memory_console', { action: 'set_user_name', name })
    if (ok) await load()
    setBusyName(false)
  }

  const forgetName = async () => {
    if (busyName) return
    setBusyName(true)
    const ok = await runTool('memory_console', { action: 'forget_user_name' })
    if (ok) {
      setNewName('')
      await load()
    }
    setBusyName(false)
  }

  const forgetPerson = async (name: string) => {
    const ok = await runTool('memory_console', { action: 'forget_person', name })
    if (ok) await load()
  }

  const deleteRelationship = async (id: string) => {
    const ok = await runTool('memory_console', { action: 'delete_relationship', id })
    if (ok) await load()
  }

  const exportJson = () => {
    if (!snapshot?.graph) return
    setBusyExport(true)
    try {
      downloadSocialGraph(snapshot.graph)
      useUIStore.getState().addNotification('success', t('settings.memoryConsole.export.success'))
    } catch (err) {
      rendererLog.error('export social memory graph failed', { error: err })
      useUIStore.getState().addNotification('error', t('settings.memoryConsole.export.failed'))
    } finally {
      setBusyExport(false)
    }
  }

  const graph = snapshot?.graph
  const people = graph?.people ?? []
  const relationships = (graph?.relationships ?? []).filter((r) => r.direction === 'user_to_person')
  const facts = graph?.facts ?? []
  const hasData = episodic.length > 0 || people.length > 0 || relationships.length > 0 || facts.length > 0

  return {
    loading,
    error,
    snapshot,
    episodic,
    searchQuery,
    setSearchQuery,
    searchedTerm,
    newFactText,
    setNewFactText,
    factFilter,
    setFactFilter,
    newName,
    setNewName,
    busyFact,
    busyName,
    busyExport,
    load,
    searchEpisodic,
    deleteEpisodic,
    addFact,
    deleteFact,
    saveName,
    forgetName,
    forgetPerson,
    deleteRelationship,
    exportJson,
    graph,
    people,
    relationships,
    facts,
    hasData,
  }
}
