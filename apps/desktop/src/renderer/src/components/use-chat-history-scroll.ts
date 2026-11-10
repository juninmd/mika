import { type RefObject, useEffect, useRef } from 'react'

export function useHistoryAutoScroll(
  scrollRef: RefObject<HTMLDivElement | null>,
  search: string,
  historyLength: number,
) {
  const isNearBottomRef = useRef(true)

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const handleScroll = () => {
      const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      isNearBottomRef.current = distanceFromBottom < 150
    }
    el.addEventListener('scroll', handleScroll, { passive: true })
    return () => el.removeEventListener('scroll', handleScroll)
  }, [scrollRef])

  // biome-ignore lint/correctness/useExhaustiveDependencies: historyLength is a trigger-only dep — it re-runs the scroll-to-bottom on new messages even though the body reads scrollRef, not history.
  useEffect(() => {
    if (search) return
    if (!isNearBottomRef.current) return
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [search, historyLength])
}

export function useHistoryEscape(search: string, setSearch: (value: string) => void, onClose: () => void) {
  const searchValueRef = useRef(search)
  searchValueRef.current = search

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (searchValueRef.current) {
          setSearch('')
        } else {
          onClose()
        }
      }
    }
    window.addEventListener('keydown', handleEscape)
    return () => window.removeEventListener('keydown', handleEscape)
  }, [onClose, setSearch])
}
