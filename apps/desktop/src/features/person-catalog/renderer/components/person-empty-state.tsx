import { Eye, type LucideIcon, MessageCircle, PenLine, Users } from 'lucide-react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'

export function PersonEmptyState() {
  const { t } = useI18n()
  const steps: { icon: LucideIcon; title: string; text: string }[] = [
    {
      icon: Eye,
      title: t('settings.personCatalog.empty.step1Title'),
      text: t('settings.personCatalog.empty.step1Text'),
    },
    {
      icon: MessageCircle,
      title: t('settings.personCatalog.empty.step2Title'),
      text: t('settings.personCatalog.empty.step2Text'),
    },
    {
      icon: PenLine,
      title: t('settings.personCatalog.empty.step3Title'),
      text: t('settings.personCatalog.empty.step3Text'),
    },
  ]

  return (
    <div className="rounded-2xl border border-dashed border-border-subtle bg-surface-raise px-6 py-12 text-center">
      <Users aria-hidden="true" className="mx-auto size-12 text-text-muted" />
      <h2 className="mt-4 text-base font-bold text-text-main">{t('settings.personCatalog.empty.title')}</h2>
      <p className="mx-auto mt-2 max-w-lg text-xs leading-relaxed text-text-muted">
        {t('settings.personCatalog.empty.hint')}
      </p>
      <div className="mx-auto mt-8 grid max-w-2xl gap-3 sm:grid-cols-3">
        {steps.map((step) => (
          <div key={step.title} className="rounded-xl border border-border-subtle bg-surface-raise/40 p-3 text-left">
            <step.icon aria-hidden="true" className="size-5 text-text-muted" />
            <p className="mt-1 text-caption font-bold text-text-main">{step.title}</p>
            <p className="mt-1 text-caption leading-relaxed text-text-muted">{step.text}</p>
          </div>
        ))}
      </div>
    </div>
  )
}
