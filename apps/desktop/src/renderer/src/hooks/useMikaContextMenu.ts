import { useCallback, useState } from 'react'
import type { ContextMenuAnchor } from '../components/MikaContextMenu'

export function useMikaContextMenu() {
  const [anchor, setAnchor] = useState<ContextMenuAnchor | null>(null)

  const open = useCallback((event: React.MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    setAnchor({ x: event.clientX, y: event.clientY })
  }, [])

  const close = useCallback(() => setAnchor(null), [])

  return { anchor, open, close }
}
