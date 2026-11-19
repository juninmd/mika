import { toDisplayError } from '@mika/sdk-ui'
import { Upload } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../../hooks/useI18n'
import { getElectronApi } from '../../../lib/shell-api'
import { useUIStore } from '../../../store/useUIStore'

type ImportModelButtonProps = {
  save: (path: string, value: unknown) => Promise<void>
}

// Rust opens the native picker and copies the model into the profile; we only apply the path it returns.
export function ImportModelButton({ save }: ImportModelButtonProps) {
  const { t } = useI18n()
  const [busy, setBusy] = useState(false)

  const importModel = async () => {
    setBusy(true)
    try {
      const pick = getElectronApi().shell.system?.importAvatarModel
      if (!pick) throw new Error(t('settings.skin.import.error'))
      const imported = await pick()
      if (!imported) return
      await save('character.modelPath', imported.modelPath)
      useUIStore.getState().addNotification('success', t('settings.skin.import.success'))
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.skin.import.error')))
    } finally {
      setBusy(false)
    }
  }

  return (
    <button
      type="button"
      disabled={busy}
      aria-busy={busy}
      onClick={() => void importModel()}
      className="inline-flex h-9 self-start items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-3 text-xs font-semibold text-text-main transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-60"
    >
      <Upload className="size-3.5" aria-hidden="true" />
      {t('settings.skin.import.button')}
    </button>
  )
}
