import { t } from '@mika/sdk/i18n'
import { AnimatePresence, motion } from 'framer-motion'
import type { LucideIcon } from 'lucide-react'
import { ChevronDown } from 'lucide-react'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { createUiLogger } from '../logger'
import { DURATION, EASE, REVEAL } from '../motion'
import { accentVars } from '../utils/accent'
import { AccentProvider, useAccent } from './AccentContext'
import { StatusPill } from './StatusPill'

const log = createUiLogger('ui:glass-section')

type TestAction = { label?: string; onTest: () => Promise<void> }

function SectionHeaderText({
  title,
  badge,
  description,
}: {
  title: string | ReactNode
  badge?: string
  description?: string
}) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-sm font-semibold text-text-main">{title}</h3>
        {badge && <StatusPill variant="neutral" label={badge} />}
      </div>
      {description && <p className="mt-0.5 text-caption leading-snug text-text-muted">{description}</p>}
    </>
  )
}

interface GlassSectionProps {
  title: string | ReactNode
  icon?: LucideIcon
  badge?: string
  description?: string
  accent?: string | number
  accentHue?: number
  testAction?: TestAction
  /** Botões do cabeçalho (atualizar, exportar, novo) — à direita, antes do teste e do colapso. */
  actions?: ReactNode
  collapsible?: boolean
  defaultCollapsed?: boolean
  children: ReactNode
  className?: string
}

export function GlassSection({
  title,
  icon: Icon,
  badge,
  description,
  accent,
  accentHue,
  testAction,
  actions,
  collapsible = false,
  defaultCollapsed = false,
  children,
  className = '',
}: GlassSectionProps) {
  const explicitHue = accentHue ?? (accent !== undefined ? Number(accent) : undefined)
  const inheritedHue = useAccent()
  const effectiveHue = explicitHue ?? inheritedHue
  const [collapsed, setCollapsed] = useState(defaultCollapsed)
  const [testing, setTesting] = useState(false)
  const titleText = typeof title === 'string' ? title : t('sdkUi.section.defaultTitle')
  const isMounted = useRef(false)

  useEffect(() => {
    setCollapsed(defaultCollapsed)
  }, [defaultCollapsed])

  useEffect(() => {
    isMounted.current = true
    return () => {
      isMounted.current = false
    }
  }, [])

  const handleTest = async () => {
    setTesting(true)
    try {
      await testAction?.onTest()
    } catch (error) {
      log.error('test action failed', { error: String(error) })
    } finally {
      if (isMounted.current) {
        setTesting(false)
      }
    }
  }

  const headerContent = <SectionHeaderText title={title} badge={badge} description={description} />

  return (
    <motion.section
      // Herda `hidden`/`show` da página (entrada escalonada); fora de um grupo, renderiza estática.
      variants={REVEAL.item}
      className={`overflow-hidden rounded-2xl border border-border-subtle bg-bg-surface backdrop-blur-sm ${className}`}
      style={{
        ...(explicitHue !== undefined ? accentVars(explicitHue) : undefined),
        boxShadow: '0 18px 44px rgba(15,23,42,0.14), inset 0 1px 0 var(--border-subtle)',
      }}
    >
      <div className="flex flex-wrap items-center gap-3 px-5 py-4">
        {Icon && (
          <div
            aria-hidden="true"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent"
          >
            <Icon size={16} />
          </div>
        )}
        {collapsible ? (
          <button
            type="button"
            className="min-w-0 flex-1 cursor-pointer select-none text-left"
            aria-expanded={!collapsed}
            onClick={() => setCollapsed((p) => !p)}
          >
            {headerContent}
          </button>
        ) : (
          <div className="min-w-0 flex-1">{headerContent}</div>
        )}
        <div className="flex flex-wrap items-center gap-2">
          {actions}
          {testAction && (
            <button
              type="button"
              onClick={() => void handleTest()}
              disabled={testing}
              className="rounded-lg border border-border-subtle bg-surface-raise px-3 py-1.5 text-caption font-medium text-text-muted transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-50"
            >
              {testing ? t('sdkUi.section.testing') : (testAction.label ?? t('sdkUi.section.testAction'))}
            </button>
          )}
          {collapsible && (
            <button
              type="button"
              aria-expanded={!collapsed}
              aria-label={
                collapsed
                  ? t('sdkUi.section.expand').replace('{{name}}', titleText)
                  : t('sdkUi.section.collapse').replace('{{name}}', titleText)
              }
              onClick={() => setCollapsed((p) => !p)}
              className="rounded-lg border border-border-subtle bg-surface-raise p-1.5 text-text-muted transition-colors hover:bg-surface-raise-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <motion.div
                animate={{ rotate: collapsed ? -90 : 0 }}
                transition={{ duration: DURATION.fast, ease: EASE.out }}
              >
                <ChevronDown size={14} aria-hidden="true" />
              </motion.div>
            </button>
          )}
        </div>
      </div>

      <AnimatePresence initial={false}>
        {!collapsed && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: DURATION.slow, ease: EASE.out }}
            style={{ overflow: 'hidden' }}
          >
            <div className="border-t border-border-subtle px-5 pb-5 pt-4">
              <AccentProvider hue={effectiveHue}>{children}</AccentProvider>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
  )
}
