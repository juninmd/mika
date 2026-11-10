import type React from 'react'
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'

interface ShadowDOMProps {
  children: React.ReactNode
}

export function ShadowDOM({ children }: ShadowDOMProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const [shadowRoot, setShadowRoot] = useState<ShadowRoot | null>(null)

  const isTestEnv = typeof process !== 'undefined' && process.env.NODE_ENV === 'test'

  useEffect(() => {
    const host = hostRef.current
    if (isTestEnv || !host) return
    // StrictMode runs this effect twice on the same host, and attachShadow throws on a second call.
    const root = host.shadowRoot ?? host.attachShadow({ mode: 'open' })

    const syncStyles = () => {
      for (const el of root.querySelectorAll('style, link[rel="stylesheet"]')) el.remove()
      for (const style of document.querySelectorAll('style, link[rel="stylesheet"]')) {
        root.appendChild(style.cloneNode(true))
      }
    }

    syncStyles()
    const observer = new MutationObserver(syncStyles)
    observer.observe(document.head, { childList: true, subtree: true })
    setShadowRoot(root)

    // Unconditional and keyed only on the env: re-running on `shadowRoot` used to tear the observer down
    // right after setup, so later <style> changes (theme switch, HMR) never reached the shadow tree.
    return () => observer.disconnect()
  }, [isTestEnv])

  if (isTestEnv) {
    return <>{children}</>
  }

  return (
    <div ref={hostRef} className="contents" style={{ display: 'contents' }}>
      {shadowRoot && createPortal(children, shadowRoot)}
    </div>
  )
}
