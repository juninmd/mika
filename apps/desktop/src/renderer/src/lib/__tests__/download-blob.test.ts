import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { downloadBlobAs, REVOKE_DELAY_MS } from '../download-blob'

describe('downloadBlobAs', () => {
  const anchor = { href: '', download: '', click: vi.fn(), remove: vi.fn() }
  const createObjectURL = vi.fn(() => 'blob:mika-1')
  const revokeObjectURL = vi.fn()

  beforeEach(() => {
    vi.useFakeTimers()
    anchor.click.mockClear()
    anchor.remove.mockClear()
    createObjectURL.mockClear()
    revokeObjectURL.mockClear()
    vi.stubGlobal('document', { createElement: () => anchor, body: { appendChild: vi.fn() } })
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL })
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  // Regression: five export paths revoked the object URL synchronously after `click()`, which can cancel the
  // save before the browser has read the blob (empty or missing file in WebKitGTK/WKWebView).
  it('clicks a named download link and revokes the URL only after a delay', () => {
    downloadBlobAs(new Blob(['x']), 'mika-chat.json')

    expect(anchor.href).toBe('blob:mika-1')
    expect(anchor.download).toBe('mika-chat.json')
    expect(anchor.click).toHaveBeenCalledTimes(1)
    expect(anchor.remove).toHaveBeenCalledTimes(1)
    expect(revokeObjectURL).not.toHaveBeenCalled()

    vi.advanceTimersByTime(REVOKE_DELAY_MS)
    expect(revokeObjectURL).toHaveBeenCalledExactlyOnceWith('blob:mika-1')
  })
})
