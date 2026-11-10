import type { CrisisRegion } from '@mika/sdk/contracts'
import { HeartHandshake, X } from 'lucide-react'
import { useI18n } from '../hooks/useI18n'
import { layerStyle } from '../lib/layer-styles'
import { useChatStore } from '../store/useChatStore'

/** Literal keys (not a template) so the i18n gates can see every region line is consumed. */
const REGION_LINE = {
  br: 'chat.crisis.region.br',
  us: 'chat.crisis.region.us',
  eu: 'chat.crisis.region.eu',
  jp: 'chat.crisis.region.jp',
  cn: 'chat.crisis.region.cn',
  kr: 'chat.crisis.region.kr',
  ru: 'chat.crisis.region.ru',
} as const satisfies Record<CrisisRegion, string>

/**
 * Discreet card with crisis resources, shown when the user's message carried a self-harm signal
 * (`brain/crisis.rs` sets `crisisRegion` on the `chat-send` response). It never blocks the chat:
 * Mika's reply still arrives, and the card is dismissible. Nothing about the message is stored.
 */
export function CrisisResourcesCard() {
  const { t } = useI18n()
  const region = useChatStore((s) => s.crisisRegion)
  const dismiss = useChatStore((s) => s.setCrisisRegion)
  if (!region) return null

  return (
    <section
      role="status"
      aria-label={t('chat.crisis.title')}
      className="hud-glass-strong fixed bottom-4 left-1/2 flex w-[min(92vw,26rem)] -translate-x-1/2 gap-3 rounded-2xl p-3.5"
      style={layerStyle('toasts')}
    >
      <span aria-hidden="true" className="mt-0.5 shrink-0 text-accent">
        <HeartHandshake size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <h2 className="text-caption font-bold uppercase tracking-[0.12em] text-text-main">{t('chat.crisis.title')}</h2>
        <p className="mt-1 text-caption leading-snug text-text-muted">{t('chat.crisis.body')}</p>
        <p className="mt-1.5 text-caption font-semibold leading-snug text-text-main">{t(REGION_LINE[region])}</p>
        <p className="mt-1 text-micro leading-snug text-text-muted">{t('chat.crisis.elsewhere')}</p>
      </div>
      <button
        type="button"
        onClick={() => dismiss(null)}
        aria-label={t('chat.crisis.dismiss')}
        className="flex h-8 w-8 shrink-0 items-center justify-center self-start rounded-lg text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <X size={14} aria-hidden="true" />
      </button>
    </section>
  )
}
