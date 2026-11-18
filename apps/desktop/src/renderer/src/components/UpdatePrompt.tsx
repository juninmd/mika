import { MOTION } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import type { useUpdateChecker } from '../hooks/useUpdateChecker'
import type { HudRect } from './hud/hud-layout'

type Props = {
  update: ReturnType<typeof useUpdateChecker>
  /** Toast column from `hud-layout`: clear of her face, where she "hands" the question over. */
  anchor: HudRect
}

/**
 * The question Mika asks out loud when she finds a new version (`update/announce.rs` speaks it):
 * nothing installs until the user says so here. "Later" hides it for this version only.
 */
export function UpdatePrompt({ update, anchor }: Props) {
  const { t } = useI18n()
  const [dismissed, setDismissed] = useState<string | null>(null)
  const { available, version, channel, downloaded, downloading, installing, error } = update
  const open = channel === 'direct' && available && version !== null && dismissed !== version
  const busy = downloading || installing

  const updateNow = async () => {
    if (downloaded || (await update.downloadUpdate())) await update.installUpdate()
  }

  return (
    <AnimatePresence>
      {open && (
        <motion.section
          key={`update-${version}`}
          role="alertdialog"
          aria-labelledby="update-prompt-title"
          aria-describedby="update-prompt-body"
          data-testid="update-prompt"
          className="hud-glass-strong pointer-events-auto fixed rounded-xl p-4"
          style={{ zIndex: 'var(--layer-toasts)', left: anchor.x, top: anchor.y, width: anchor.width }}
          {...MOTION.toast}
        >
          <h2 id="update-prompt-title" className="text-micro font-bold uppercase tracking-[0.12em] text-accent">
            {t('shell.update.prompt.title')}
          </h2>
          <p id="update-prompt-body" className="mt-1 text-caption leading-snug text-text-main">
            {t('shell.update.prompt.body', { version })}
          </p>
          {error && (
            <p role="alert" className="mt-2 text-caption text-danger">
              {error}
            </p>
          )}
          <div className="mt-3 flex gap-2">
            <button
              type="button"
              onClick={() => setDismissed(version)}
              disabled={busy}
              className="flex-1 rounded-lg border border-border-subtle px-3 py-1.5 text-caption font-medium text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('shell.update.prompt.later')}
            </button>
            <button
              type="button"
              onClick={() => void updateNow()}
              disabled={busy}
              aria-busy={busy}
              className="flex-1 rounded-lg border border-accent-rim bg-accent-soft px-3 py-1.5 text-caption font-semibold text-accent transition-[filter] hover:brightness-110 disabled:opacity-60 focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {busy ? t('shell.update.prompt.updating') : t('shell.update.prompt.now')}
            </button>
          </div>
        </motion.section>
      )}
    </AnimatePresence>
  )
}
