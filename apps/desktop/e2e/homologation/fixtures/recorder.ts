import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ToolResult } from './homolog-app'

// Homologation verdict: 'falha' is the only one that blames Mika; 'sem-credencial' and
// 'bloqueado-externo' separate "doesn't work" from "can't prove from here" — treating all three
// as red made the previous homologation read as noise and get ignored.
export type Verdict = 'ok' | 'sem-credencial' | 'bloqueado-externo' | 'falha'

export type CheckResult = { feature: string; check: string; verdict: Verdict; detail: string }

export function summarize(result: ToolResult): string {
  if (result?.error) return `error: ${result.error}`
  return `data: ${JSON.stringify(result?.data ?? result).slice(0, 200)}`
}

export function looksLikeMissingCredential(result: ToolResult): boolean {
  const text = `${result?.error ?? ''}`.toLowerCase()
  return /token|api ?key|configure|não configurad|not configured|credential|steamid|bot ?token/.test(text)
}

// Refusal by third-party POLICY (e.g. ZDR), not missing credentials — else it would send someone chasing the wrong key.
export function looksLikeExternalBlock(result: ToolResult): boolean {
  const text = `${result?.error ?? ''}`.toLowerCase()
  return /data policy|zero data retention|no endpoints found/.test(text)
}

export interface Recorder {
  results: CheckResult[]
  record: (feature: string, check: string, verdict: Verdict, detail: string) => void
  // Records 'ok' when the tool didn't return success:false; anything else is 'falha'.
  probe: (feature: string, check: string, result: ToolResult) => ToolResult
  flush: () => void
}

// Identifies what the report was run against; without it two runs collide on the same file.
function runLabel(): { alvo: string; sufixo: string } {
  const provider = process.env.MIKA_HOMOLOG_PROVIDER ?? 'ollama'
  const model = process.env.MIKA_HOMOLOG_MODEL ?? (provider === 'ollama' ? 'qwen3.5:4b' : 'google/gemma-4-31b-it:free')
  const tag = process.env.MIKA_HOMOLOG_RUN
  return { alvo: `${provider} · ${model}`, sufixo: tag ? `-${tag}` : '' }
}

function classify(result: ToolResult): Verdict {
  if (result?.success !== false) return 'ok'
  if (looksLikeExternalBlock(result)) return 'bloqueado-externo'
  return looksLikeMissingCredential(result) ? 'sem-credencial' : 'falha'
}

export function createRecorder(title: string, fileBase: string): Recorder {
  const results: CheckResult[] = []

  const record: Recorder['record'] = (feature, check, verdict, detail) => {
    results.push({ feature, check, verdict, detail: detail.replace(/\s+/g, ' ').slice(0, 220) })
  }

  return {
    results,
    record,
    probe: (feature, check, result) => {
      const verdict: Verdict = classify(result)
      record(feature, check, verdict, summarize(result))
      return result
    },
    flush: () => {
      const outDir = join(__dirname, '..', 'report')
      mkdirSync(outDir, { recursive: true })
      const { alvo, sufixo } = runLabel()
      const count = (v: Verdict) => results.filter((r) => r.verdict === v).length
      const lines = [
        `# ${title}`,
        '',
        `Executada contra o app real, modelo \`${alvo}\`. ${results.length} verificações: `,
        `**${count('ok')} ok**, **${count('sem-credencial')} sem credencial**, ` +
          `**${count('bloqueado-externo')} bloqueado por terceiro**, **${count('falha')} falha**.`,
        '',
        '| Feature | Verificação | Veredito | Detalhe |',
        '| --- | --- | --- | --- |',
        ...results.map((r) => `| ${r.feature} | ${r.check} | ${r.verdict} | ${r.detail.replace(/\|/g, '\|')} |`),
        '',
      ]
      writeFileSync(join(outDir, `${fileBase}${sufixo}.md`), lines.join('\n'))
      writeFileSync(join(outDir, `${fileBase}${sufixo}.json`), JSON.stringify(results, null, 2))

      const falhas = results.filter((r) => r.verdict === 'falha')
      console.log(`\n${fileBase}${sufixo} (${alvo}): ${results.length} verificações, ${falhas.length} falhas`)
      for (const f of falhas) console.log(`  FALHA ${f.feature} — ${f.check}: ${f.detail}`)
    },
  }
}
