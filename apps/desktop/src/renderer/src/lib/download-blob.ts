/** Long enough for the browser to have started the save: revoking right after `click()` can abort the download. */
export const REVOKE_DELAY_MS = 60_000

/** Saves `blob` through a throwaway `<a download>`; the object URL is released after `REVOKE_DELAY_MS`. */
export function downloadBlobAs(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), REVOKE_DELAY_MS)
}
