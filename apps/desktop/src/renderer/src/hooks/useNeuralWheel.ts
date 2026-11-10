import { useCallback, useEffect, useState } from 'react'

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  return target.tagName === 'INPUT' || target.tagName === 'TEXTAREA'
}

export function useNeuralWheel() {
  const [isOpen, setIsOpen] = useState(false)
  const [position, setPosition] = useState({ x: 0, y: 0 })

  const openWheel = useCallback((pos?: { x: number; y: number }) => {
    setPosition(pos ?? { x: window.innerWidth / 2, y: window.innerHeight / 2 })
    setIsOpen(true)
  }, [])

  const handleMouseDown = useCallback(
    (e: MouseEvent) => {
      if (e.button !== 1) return
      if (e.ctrlKey || e.altKey || e.shiftKey) return
      if (isEditableTarget(e.target)) return

      e.preventDefault()
      openWheel({ x: e.clientX, y: e.clientY })
    },
    [openWheel],
  )

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!e.ctrlKey || !e.shiftKey || e.code !== 'Space') return
      if (isEditableTarget(e.target)) return

      e.preventDefault()
      openWheel()
    },
    [openWheel],
  )

  const closeWheel = useCallback(() => setIsOpen(false), [])

  useEffect(() => {
    window.addEventListener('mousedown', handleMouseDown)
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('mousedown', handleMouseDown)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleMouseDown, handleKeyDown])

  useEffect(() => {
    if (!isOpen) return
    window.addEventListener('blur', closeWheel)
    return () => window.removeEventListener('blur', closeWheel)
  }, [isOpen, closeWheel])

  return {
    isOpen,
    position,
    openWheel,
    closeWheel,
  }
}
