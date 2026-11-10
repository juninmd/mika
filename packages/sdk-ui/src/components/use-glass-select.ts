import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createUiLogger } from '../logger'
import { useAccent } from './AccentContext'
import {
  computePosition,
  ESTIMATED_PANEL_HEIGHT,
  filterOptions,
  type GlassSelectProps,
  nextIndexFor,
  type PanelPosition,
  readCurrentValue,
} from './glass-select-helpers'

const log = createUiLogger('ui:glass-select')

export function useGlassSelect(props: GlassSelectProps) {
  const { configPath, options, onSave, searchable, accentHue: accentHueProp, onChange } = props
  const accentHue = useAccent(accentHueProp)
  const selectId = useId()
  const listboxId = `${selectId}-listbox`
  const [isOpen, setIsOpen] = useState(false)
  const [search, setSearchState] = useState('')
  const [activeIndex, setActiveIndex] = useState(0)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [position, setPosition] = useState<PanelPosition | null>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  // Only keyboard-driven activeIndex changes (and the initial open) should move DOM
  // focus; mouse hover must only update the visual highlight, never steal focus.
  const focusOnActiveChangeRef = useRef(true)

  const currentValue = readCurrentValue(props)
  const selectedOption = options.find((o) => o.value === currentValue)

  const filteredOptions = searchable ? filterOptions(options, search) : options

  const close = useCallback((returnFocus: boolean) => {
    setIsOpen(false)
    setSearchState('')
    if (returnFocus) triggerRef.current?.focus()
  }, [])

  const open = useCallback(() => {
    const index = options.findIndex((o) => o.value === currentValue)
    focusOnActiveChangeRef.current = true
    setActiveIndex(Math.max(index, 0))
    setIsOpen(true)
  }, [options, currentValue])

  // setSearch guarda a invariante do painel: o índice ativo nunca sai da lista filtrada, mesmo
  // quando quem chama o hook não o recompõe depois (o GlassSelect reseta junto, mas a garantia
  // é do hook que exporta o setSearch).
  const setSearch = useCallback(
    (next: string) => {
      setSearchState(next)
      const lastIndex = Math.max(0, filterOptions(options, next).length - 1)
      setActiveIndex((prev) => Math.min(prev, lastIndex))
    },
    [options],
  )

  const updatePosition = useCallback(() => {
    const trigger = triggerRef.current
    if (!trigger) return
    setPosition(
      computePosition(trigger.getBoundingClientRect(), panelRef.current?.offsetHeight || ESTIMATED_PANEL_HEIGHT),
    )
  }, [])

  // biome-ignore lint/correctness/useExhaustiveDependencies: filteredOptions.length is a trigger-only dep — it re-measures panel position when the option count (and thus panel height) changes, even though the body doesn't read filteredOptions.
  useLayoutEffect(() => {
    if (!isOpen) return
    updatePosition()

    window.addEventListener('scroll', updatePosition, true)
    window.addEventListener('resize', updatePosition)
    return () => {
      window.removeEventListener('scroll', updatePosition, true)
      window.removeEventListener('resize', updatePosition)
    }
  }, [isOpen, filteredOptions.length, updatePosition])

  // biome-ignore lint/correctness/useExhaustiveDependencies: activeIndex is a trigger-only dep — it re-focuses/scrolls the newly active option, even though the body reads the DOM via a data-attribute query, not activeIndex directly.
  useEffect(() => {
    if (!isOpen) return

    const active = panelRef.current?.querySelector<HTMLElement>('[data-active="true"]')
    if (searchable) {
      active?.scrollIntoView?.({ block: 'nearest' })
      return
    }
    if (!focusOnActiveChangeRef.current) return
    if (active) active.focus()
    else panelRef.current?.focus()
  }, [isOpen, searchable, activeIndex])

  useEffect(() => {
    if (!isOpen || !searchable) return
    panelRef.current?.querySelector<HTMLInputElement>('input')?.focus()
  }, [isOpen, searchable])

  useEffect(() => {
    if (!isOpen) return
    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node

      if (triggerRef.current?.contains(target) || panelRef.current?.contains(target)) return
      close(false)
    }
    document.addEventListener('mousedown', handlePointerDown)
    return () => document.removeEventListener('mousedown', handlePointerDown)
  }, [isOpen, close])

  const selectOption = useCallback(
    (next: string) => {
      onChange?.(next)
      if (onSave && configPath) {
        onSave(configPath, next)
          .then(() => setSaveError(null))
          .catch((error: unknown) => {
            const message = toErrorMessage(error)
            setSaveError(message)
            log.error('failed to save setting', { configPath, error: message })
          })
      }
      close(true)
    },
    [close, configPath, onSave, onChange],
  )

  const handlePanelKeyDown = (event: React.KeyboardEvent) => {
    const next = nextIndexFor(event.key, activeIndex, Math.max(0, filteredOptions.length - 1))
    if (next !== null) {
      event.preventDefault()
      focusOnActiveChangeRef.current = true
      setActiveIndex(next)
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      close(true)
      return
    }

    if (event.key === 'Tab') {
      close(false)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      const option = filteredOptions[activeIndex]
      if (option) selectOption(option.value)
    }
  }

  const activeDescendant = filteredOptions[activeIndex] ? `${listboxId}-opt-${activeIndex}` : undefined

  return {
    accentHue,
    selectId,
    listboxId,
    isOpen,
    search,
    setSearch,
    activeIndex,
    setActiveIndex,
    saveError,
    position,
    triggerRef,
    panelRef,
    focusOnActiveChangeRef,
    currentValue,
    selectedOption,
    filteredOptions,
    close,
    open,
    selectOption,
    handlePanelKeyDown,
    activeDescendant,
  }
}
