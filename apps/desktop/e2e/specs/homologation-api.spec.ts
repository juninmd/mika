import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'
import { API_CALL_ARGS, API_UNSAFE } from './homologation-api.cases'

type CallReport = {
  path: string
  kind: 'invoke' | 'subscription' | 'skipped'
  ok: boolean
  detail: string
}

test('homologação: toda chamada de API exposta responde', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const reports = await page.evaluate(
    async ({ args, unsafe }: { args: Record<string, unknown[]>; unsafe: string[] }) => {
      function shapeOf(value: unknown): string {
        if (value === undefined) return 'undefined'
        if (value === null) return 'null'
        if (Array.isArray(value)) return `array(${value.length})`
        if (value instanceof Date) return 'Date'
        if (typeof value !== 'object') return typeof value
        const entries = Object.entries(value as Record<string, unknown>)
        if (entries.length === 0) return '{} VAZIO'
        return entries
          .map(([k, v]) => {
            if (Array.isArray(v)) return `${k}(${v.length})`
            if (v === null) return `${k}=null`
            if (v === '') return `${k}=''`
            if (v instanceof Date) return k
            if (typeof v === 'object' && Object.keys(v as object).length === 0) return `${k}{}`
            return k
          })
          .slice(0, 10)
          .join(', ')
      }

      const api = (window as unknown as { electronAPI?: Record<string, unknown> }).electronAPI
      if (!api) return [{ path: 'window.electronAPI', kind: 'invoke', ok: false, detail: 'não exposta' }]

      const out: Array<{ path: string; kind: string; ok: boolean; detail: string }> = []
      const unsafeSet = new Set(unsafe)

      async function callOne(path: string, fn: (...a: unknown[]) => unknown): Promise<void> {
        if (unsafeSet.has(path)) {
          out.push({ path, kind: 'skipped', ok: true, detail: 'declarado em API_UNSAFE' })
          return
        }
        const isSubscription = /^on[A-Z]/.test(path.split('.').pop() ?? '')
        try {
          const result = isSubscription ? fn(() => {}) : await fn(...(args[path] ?? []))
          if (isSubscription) {
            const ok = typeof result === 'function'
            if (ok) (result as () => void)()
            out.push({
              path,
              kind: 'subscription',
              ok,
              detail: ok ? 'devolveu cancelamento' : `devolveu ${typeof result}, esperado function`,
            })
            return
          }

          out.push({ path, kind: 'invoke', ok: true, detail: shapeOf(result) })
        } catch (err) {
          out.push({
            path,
            kind: isSubscription ? 'subscription' : 'invoke',
            ok: false,
            detail: err instanceof Error ? err.message : String(err),
          })
        }
      }

      async function walk(prefix: string, node: Record<string, unknown>, depth: number): Promise<void> {
        for (const [name, member] of Object.entries(node)) {
          const path = prefix ? `${prefix}.${name}` : name
          if (typeof member === 'function') {
            await callOne(path, member as (...a: unknown[]) => unknown)
          } else if (member && typeof member === 'object' && depth < 2) {
            await walk(path, member as Record<string, unknown>, depth + 1)
          }
        }
      }

      await walk('', api, 0)
      return out
    },
    { args: API_CALL_ARGS, unsafe: API_UNSAFE },
  )

  const seen = new Set((reports as CallReport[]).map((r) => r.path))

  const staleUnsafe = API_UNSAFE.filter((path) => !seen.has(path))
  const staleArgs = Object.keys(API_CALL_ARGS).filter((path) => !seen.has(path))

  const typed = reports as CallReport[]
  for (const r of typed) {
    const mark = r.kind === 'skipped' ? '⏭️ ' : r.ok ? '✅' : '❌'
    console.log(`${mark} ${r.path} — ${r.detail}`)
  }

  const executed = typed.filter((r) => r.kind !== 'skipped')
  const failures = typed.filter((r) => !r.ok)

  console.log(
    `\n${executed.length} chamadas executadas · ${typed.filter((r) => r.kind === 'subscription').length} assinaturas · ${typed.length - executed.length} puladas`,
  )

  expect(executed.length).toBeGreaterThan(50)
  expect(failures.map((f) => `${f.path}: ${f.detail}`)).toEqual([])
  expect(staleUnsafe, 'em API_UNSAFE e sem método correspondente').toEqual([])
  expect(staleArgs, 'em API_CALL_ARGS e sem método correspondente').toEqual([])
  writeArtifact('homologation-api-report', { executed: executed.length, skipped: typed.length - executed.length, reports: typed })
})
