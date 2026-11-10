import { toDisplayError } from '@mika/sdk-ui'
import { useAccent } from '@mika/sdk-ui/settings'
import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'

type MermaidDiagramProps = {
  chart: string
  accentHue?: number
  className?: string
  title?: string
  description?: string
  legend?: Array<{ label: string; value: string }>
}

const rendered = new Map<string, string>()
// In flight by key: leaving the tab mid-render used to throw the work away (nothing cached), so every
// return to the tab paid mermaid + DOMPurify again (~300 ms at 4x CPU throttle). Now the render always
// finishes into the cache and a remount just awaits it.
const inflight = new Map<string, Promise<{ svg?: string; error?: string }>>()

// DOMPurify (60 KB) loads with mermaid, on first diagram — it used to ride in every settings open.
async function sanitizeDiagramSvg(svg: string): Promise<string> {
  const DOMPurify = (await import('dompurify')).default
  const sanitized = DOMPurify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    FORBID_TAGS: ['foreignObject', 'foreignobject'],
  })
  const document = new DOMParser().parseFromString(sanitized, 'image/svg+xml')
  for (const node of document.querySelectorAll('foreignObject, foreignobject')) node.remove()
  return new XMLSerializer().serializeToString(document.documentElement)
}

async function renderDiagram(opts: {
  chart: string
  key: string
  accentHue: number
  fallbackErrorMessage: string
}): Promise<{ svg?: string; error?: string }> {
  try {
    const mermaid = await import('mermaid').then((m) => m.default)
    mermaid.initialize({
      startOnLoad: false,
      // Mermaid 12 passou a usar ELK e o visual "neo" por padrão; fixa o desenho que a UI já usava.
      layout: 'dagre',
      look: 'classic',
      theme: 'dark',
      themeVariables: {
        primaryColor: `hsl(${opts.accentHue} 70% 50%)`,
        primaryTextColor: `hsl(${opts.accentHue} 20% 90%)`,
        primaryBorderColor: `hsl(${opts.accentHue} 50% 40%)`,
        lineColor: `hsl(${opts.accentHue} 40% 60%)`,
        secondaryColor: `hsl(${opts.accentHue} 30% 15%)`,
        tertiaryColor: `hsl(${opts.accentHue} 20% 10%)`,
      },
      // O SVG entra por innerHTML: 'strict' sanitiza caso o chart um dia venha do LLM.
      securityLevel: 'strict',
      fontFamily: '"IBM Plex Mono", "JetBrains Mono", ui-monospace, monospace',

      htmlLabels: false,
      flowchart: { htmlLabels: false, useMaxWidth: true },
    })
    const { svg: renderedSvg } = await mermaid.render(`mermaid-${opts.key}`, opts.chart)
    const sanitized = await sanitizeDiagramSvg(renderedSvg)
    rendered.set(opts.key, sanitized)
    return { svg: sanitized }
  } catch (e) {
    return { error: toDisplayError(e, opts.fallbackErrorMessage) }
  }
}

function hashChart(chart: string): string {
  let h = 0
  for (let i = 0; i < chart.length; i++) {
    h = (Math.imul(31, h) + chart.charCodeAt(i)) | 0
  }
  return String(Math.abs(h))
}

export function MermaidDiagram({
  chart,
  accentHue: accentHueProp,
  className = '',
  title,
  description,
  legend,
}: MermaidDiagramProps) {
  const { t } = useI18n()
  const accentHue = useAccent(accentHueProp)
  const key = `${hashChart(chart)}-${accentHue}`
  const [svg, setSvg] = useState<string | null>(() => rendered.get(key) ?? null)
  const [error, setError] = useState<string | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const cached = rendered.get(key)
    if (cached) {
      setSvg(cached)
      setError(null)
      return
    }

    let cancelled = false

    let pending = inflight.get(key)
    if (!pending) {
      pending = renderDiagram({ chart, key, accentHue, fallbackErrorMessage: t('settings.diagram.renderFailed') })
      inflight.set(key, pending)
      void pending.finally(() => inflight.delete(key))
    }
    pending.then((result) => {
      if (cancelled) return
      if (result.error) {
        setError(result.error)
      } else if (result.svg) {
        setSvg(result.svg)
        setError(null)
      }
    })

    return () => {
      cancelled = true
    }
  }, [key, chart, accentHue, t])

  if (error) {
    return (
      <div role="alert" className={`rounded-xl border border-danger-rim bg-danger-soft p-4 ${className}`}>
        <p className="text-caption font-medium text-danger">{t('settings.diagram.error')}</p>
        <pre className="mt-2 max-h-40 overflow-auto text-caption leading-relaxed text-danger/70">{error}</pre>
      </div>
    )
  }

  return (
    <div className={`rounded-xl border border-border-subtle bg-surface-raise p-4 ${className}`}>
      {title || description ? (
        <div className="mb-4 flex flex-col gap-1">
          {title ? <p className="text-caption font-black uppercase tracking-[0.12em] text-text-main">{title}</p> : null}
          {description ? <p className="text-caption leading-relaxed text-text-muted">{description}</p> : null}
        </div>
      ) : null}
      {svg ? (
        <div
          ref={containerRef}
          role="img"
          aria-label={title ?? t('settings.diagram.conceptual')}
          className="mermaid-diagram"
          // biome-ignore lint/security/noDangerouslySetInnerHtml: svg is mermaid's own SVG output run through `sanitizeDiagramSvg` (DOMPurify with FORBID_TAGS foreignObject, plus mermaid's own securityLevel: 'strict') before it ever reaches state — there is no other way to inject a pre-rendered SVG string in React.
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      ) : (
        <div role="status" aria-label={t('settings.diagram.loading')} className="flex h-32 items-center justify-center">
          <div
            className="h-4 w-4 animate-spin rounded-full border-2 border-border-strong border-t-accent"
            aria-hidden="true"
          />
        </div>
      )}
      {legend?.length ? (
        <div className="mt-4 grid gap-2 sm:grid-cols-3">
          {legend.map((item) => (
            <div key={item.label} className="rounded-lg border border-border-subtle bg-surface-raise/15 px-3 py-2">
              <p className="text-micro font-black uppercase tracking-[0.12em] text-text-muted">{item.label}</p>
              <p className="mt-1 truncate text-caption font-semibold text-text-main" title={item.value}>
                {item.value}
              </p>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
