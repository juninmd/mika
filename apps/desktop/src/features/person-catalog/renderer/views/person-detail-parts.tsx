import { formatLocalDate } from '@mika/sdk-ui'
import { User } from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import { meaningful } from '../components/person-card'
import type { PersonMention, PersonRecord, PersonRelationship } from '../person-catalog.types'

export type FullPerson = PersonRecord & { mentions: PersonMention[] }

export function fmtDate(ts: number | undefined, locale: string) {
  if (!ts) return ''
  return formatLocalDate(ts, locale, {
    day: '2-digit',
    month: 'short',
    year: '2-digit',
  })
}

function Fact({ label, value }: { label: string; value?: string }) {
  if (!meaningful(value)) return null
  return (
    <div className="bg-surface-raise rounded-lg p-2 border border-border-subtle">
      <p className="text-micro text-text-subtle font-bold uppercase mb-0.5">{label}</p>
      <p className="text-caption text-text-main leading-tight">{value}</p>
    </div>
  )
}

export function RelationshipRow({ rel }: { rel: PersonRelationship }) {
  return (
    <div className="flex items-center gap-2 bg-surface-raise rounded-lg px-2.5 py-1.5 border border-border-subtle">
      <User aria-hidden="true" className="size-4 shrink-0 text-text-muted" />
      <div className="min-w-0 flex-1">
        <p className="text-caption font-medium text-text-main truncate">{rel.name}</p>
        <p className="text-micro text-text-muted uppercase tracking-wider">{rel.type}</p>
      </div>
    </div>
  )
}

export function FactsGrid({ p, t }: { p: FullPerson; t: Translate }) {
  return (
    <div className="grid grid-cols-2 gap-2">
      {p.birthDate && <Fact label={t('settings.personCatalog.listing.factBirthDate')} value={p.birthDate} />}
      {p.birthPlace && <Fact label={t('settings.personCatalog.listing.factBirthPlace')} value={p.birthPlace} />}
      {p.gender && <Fact label={t('settings.personCatalog.listing.factGender')} value={p.gender} />}
      {p.nationality && <Fact label={t('settings.personCatalog.form.nationalityLabel')} value={p.nationality} />}
      {p.species && <Fact label={t('settings.personCatalog.listing.factSpecies')} value={p.species} />}
      {p.affiliation && <Fact label={t('settings.personCatalog.listing.factAffiliation')} value={p.affiliation} />}
      {p.voiceActor && <Fact label={t('settings.personCatalog.listing.factVoiceActor')} value={p.voiceActor} />}
      {p.role && <Fact label={t('settings.personCatalog.listing.factRole')} value={p.role} />}
      {p.origin && <Fact label={t('settings.personCatalog.form.originLabel')} value={p.origin} />}
    </div>
  )
}
