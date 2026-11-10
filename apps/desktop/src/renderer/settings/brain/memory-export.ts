import { downloadBlobAs } from '../../src/lib/download-blob'

export function downloadSocialGraph(graph: unknown): void {
  const blob = new Blob([JSON.stringify(graph, null, 2)], { type: 'application/json' })
  downloadBlobAs(blob, `mika-social-memory-${new Date().toISOString().slice(0, 10)}.json`)
}
