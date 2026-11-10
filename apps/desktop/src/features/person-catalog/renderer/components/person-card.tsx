import { formatRelativeTime } from '@mika/sdk/utils/time-format'
import { ConfirmButton } from '@mika/sdk-ui/settings'
import {
  BookOpen,
  Brain,
  Building2,
  Clapperboard,
  Drama,
  Eye,
  Gamepad2,
  HelpCircle,
  Landmark,
  Laugh,
  type LucideIcon,
  Medal,
  Mic,
  Newspaper,
  PawPrint,
  PenLine,
  Sparkles,
  Star,
  Trash2,
  Tv,
  User,
} from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import type { PersonType } from '../../../../shared/person-types'
import type { PersonRecord } from '../person-catalog.types'
import { usePersonImage } from '../use-person-image'

export type CardPerson = Pick<
  PersonRecord,
  | 'id'
  | 'primaryName'
  | 'type'
  | 'imageUrl'
  | 'tags'
  | 'mentionCount'
  | 'lastSeenAt'
  | 'origin'
  | 'enriched'
  | 'relationToUser'
  | 'facts'
>

export const TYPE_ICON: Record<string, LucideIcon> = {
  real_person: User,
  fictional_character: Drama,
  organization: Building2,
  pet: PawPrint,
  politician: Landmark,
  journalist: Newspaper,
  influencer: Star,
  athlete: Medal,
  singer: Mic,
  streamer: Tv,
  movie: Clapperboard,
  series: Tv,
  historical: Landmark,
  author: PenLine,
  comedian: Laugh,
  book: BookOpen,
  vtuber: Sparkles,
  game: Gamepad2,
  anime: Tv,
  other: HelpCircle,
}

const TYPE_LABEL_KEY: Record<PersonType, Parameters<Translate>[0]> = {
  real_person: 'settings.personCatalog.type.realPerson',
  fictional_character: 'settings.personCatalog.type.fictionalCharacter',
  organization: 'settings.personCatalog.type.organization',
  pet: 'settings.personCatalog.type.pet',
  politician: 'settings.personCatalog.type.politician',
  journalist: 'settings.personCatalog.type.journalist',
  influencer: 'settings.personCatalog.type.influencer',
  athlete: 'settings.personCatalog.type.athlete',
  singer: 'settings.personCatalog.type.singer',
  streamer: 'settings.personCatalog.type.streamer',
  movie: 'settings.personCatalog.type.movie',
  series: 'settings.personCatalog.type.series',
  historical: 'settings.personCatalog.type.historical',
  author: 'settings.personCatalog.type.author',
  comedian: 'settings.personCatalog.type.comedian',
  book: 'settings.personCatalog.type.book',
  vtuber: 'settings.personCatalog.type.vtuber',
  game: 'settings.personCatalog.type.game',
  anime: 'settings.personCatalog.type.anime',
  other: 'settings.personCatalog.type.other',
}

export function typeLabel(t: Translate, type: string): string {
  const key = TYPE_LABEL_KEY[type as PersonType]
  return key ? t(key) : type.replace(/_/g, ' ')
}

const PLACEHOLDER_VALUES = new Set(['n/a', 'na', 'undefined', 'null', 'none', 'desconhecido', '-', '?'])

export function meaningful(value?: string): boolean {
  const text = value?.trim()
  if (!text) return false
  return !PLACEHOLDER_VALUES.has(text.toLowerCase())
}

export function PersonCard({
  person,
  onSelect,
  onDelete,
  highlight = false,
  rpc,
}: {
  person: CardPerson
  onSelect: (id: string) => void
  onDelete?: (id: string) => void
  highlight?: boolean

  rpc?: { call<T>(method: string, args?: unknown): Promise<T> } | null
}) {
  const imageDataUrl = usePersonImage(rpc, person.imageUrl)
  const { t } = useI18n()

  return (
    <button
      type="button"
      onClick={(e) => {
        if ((e.target as HTMLElement).closest('[data-card-action]')) return
        onSelect(person.id)
      }}
      className={`relative text-left rounded-2xl bg-surface-raise/40 backdrop-blur-xl border overflow-hidden transition hover:bg-surface-raise hover:border-border-strong focus-visible:ring-2 focus-visible:ring-focus focus-visible:outline-none group ${
        highlight ? 'border-warn/50 bg-warn-soft' : 'border-border-subtle'
      }`}
    >
      {imageDataUrl ? (
        <img
          src={imageDataUrl}
          alt={person.primaryName}
          loading="lazy"
          decoding="async"
          className="w-full aspect-[4/3] object-cover object-top bg-surface-raise"
        />
      ) : (
        <div className="w-full aspect-[4/3] bg-surface-raise flex items-center justify-center">
          {(() => {
            const Icon = TYPE_ICON[person.type] ?? HelpCircle
            return <Icon aria-hidden="true" className="size-10 text-text-muted" />
          })()}
        </div>
      )}
      <div className="p-2.5 space-y-1">
        <div className="flex items-center justify-between gap-1">
          <p className="text-xs font-bold text-text-main truncate">{person.primaryName}</p>
          {person.lastSeenAt && (
            <span className="text-caption text-text-subtle whitespace-nowrap">
              {formatRelativeTime(person.lastSeenAt, t)}
            </span>
          )}
        </div>
        {meaningful(person.origin) && <p className="text-caption text-info/70 truncate">{person.origin}</p>}
        {meaningful(person.relationToUser) && (
          <p className="text-caption text-warn/80 truncate">{person.relationToUser}</p>
        )}
        <div className="flex items-center gap-1 flex-wrap">
          <span className="text-caption px-1.5 py-0.5 rounded bg-accent-soft text-accent/80 font-bold">
            {typeLabel(t, person.type)}
          </span>
          <span className="flex items-center gap-0.5 text-caption text-text-subtle">
            <Eye aria-hidden="true" className="size-3" />
            {person.mentionCount}
          </span>
          {(person.facts?.length ?? 0) > 0 && (
            <span
              className="flex items-center gap-0.5 text-caption text-ok"
              title={t('settings.personCatalog.memory.title')}
            >
              <Brain aria-hidden="true" className="size-3" />
              {person.facts?.length}
            </span>
          )}
        </div>
        {person.tags.length > 0 && (
          <div className="flex gap-1 flex-wrap pt-0.5">
            {person.tags.slice(0, 3).map((t) => (
              <span key={t} className="text-caption px-1 py-0.5 rounded bg-surface-raise/10 text-text-muted">
                {t}
              </span>
            ))}
            {person.tags.length > 3 && <span className="text-caption text-text-subtle">+{person.tags.length - 3}</span>}
          </div>
        )}
      </div>
      {onDelete && (
        <span
          data-card-action
          className="absolute top-1.5 right-1.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity"
        >
          <ConfirmButton
            ariaLabel={`${t('settings.personCatalog.card.deleteAriaLabelPrefix')} "${person.primaryName}" ${t('settings.personCatalog.card.deleteAriaLabelSuffix')}`}
            icon={Trash2}
            onConfirm={() => onDelete(person.id)}
          />
        </span>
      )}
    </button>
  )
}
