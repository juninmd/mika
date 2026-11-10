import { StatusPill } from '@mika/sdk-ui/settings'
import { User } from 'lucide-react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { TYPE_ICON, typeLabel } from '../components/person-card'
import { usePersonImage } from '../use-person-image'
import { FactsGrid, type FullPerson, fmtDate, RelationshipRow } from './person-detail-parts'
import { PersonMemorySection } from './person-memory-section'

export function PersonDetailView({
  person,
  onClose,
  onChanged,
  rpc,
}: {
  person: FullPerson
  onClose: () => void
  onChanged: () => void
  rpc?: { call<T>(method: string, args?: unknown): Promise<T> } | null
}) {
  const imageDataUrl = usePersonImage(rpc, person.imageUrl)
  const { t, locale } = useI18n()

  return (
    <div className="flex flex-col gap-4 max-w-2xl mx-auto w-full">
      <button
        type="button"
        onClick={onClose}
        className="self-start text-micro text-text-muted hover:text-text-main transition-colors font-bold uppercase tracking-wider focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded-lg"
      >
        {t('settings.personCatalog.listing.backToList')}
      </button>

      <div className="flex gap-4">
        {imageDataUrl ? (
          <img
            src={imageDataUrl}
            alt={person.primaryName}
            className="w-24 h-32 object-cover rounded-xl border border-border-subtle flex-shrink-0 bg-surface-raise"
          />
        ) : (
          <div className="w-24 h-32 rounded-xl border border-border-subtle bg-surface-raise flex items-center justify-center flex-shrink-0">
            {(() => {
              const Icon = TYPE_ICON[person.type] ?? User
              return <Icon aria-hidden="true" className="size-10 text-text-muted" />
            })()}
          </div>
        )}
        <div className="flex flex-col gap-1 min-w-0">
          <h2 className="text-lg font-bold text-text-main">{person.primaryName}</h2>
          <div className="flex flex-wrap gap-1">
            <StatusPill variant="accent" label={typeLabel(t, person.type)} />
            {person.relationToUser && (
              <StatusPill
                variant="warn"
                label={`${t('settings.personCatalog.listing.relationToUserPrefix')} ${person.relationToUser}`}
              />
            )}
            <StatusPill
              variant="info"
              label={`${person.mentionCount} ${t('settings.personCatalog.panel.mentionsSuffix')}`}
            />
            {person.enriched && <StatusPill variant="ok" label={t('settings.personCatalog.listing.enrichedBadge')} />}
          </div>
          {person.occupation?.length ? (
            <p className="text-caption text-text-muted">{person.occupation.join(' · ')}</p>
          ) : null}
          {person.knownFor?.length ? (
            <p className="text-caption text-text-muted">
              {t('settings.personCatalog.listing.knownForPrefix')} {person.knownFor.join(', ')}
            </p>
          ) : null}
          <div className="flex gap-4 text-caption text-text-subtle font-mono mt-auto">
            {person.firstSeenAt && (
              <span>
                {t('settings.personCatalog.listing.firstSeenPrefix')} {fmtDate(person.firstSeenAt, locale)}
              </span>
            )}
            {person.lastSeenAt && (
              <span>
                {t('settings.personCatalog.listing.lastSeenPrefix')} {fmtDate(person.lastSeenAt, locale)}
              </span>
            )}
          </div>
        </div>
      </div>

      {person.bio && (
        <div className="bg-surface-raise rounded-xl p-3 border border-border-subtle">
          <p className="text-caption text-text-main leading-relaxed">{person.bio}</p>
        </div>
      )}

      {person.userNotes && (
        <div className="bg-warn/[0.04] rounded-xl p-3 border border-warn-rim">
          <p className="text-micro font-bold text-warn/80 uppercase mb-1 tracking-wider">
            {t('settings.personCatalog.listing.userNotesTitle')}
          </p>
          <p className="text-caption text-text-main">{person.userNotes}</p>
        </div>
      )}

      {rpc && <PersonMemorySection person={person} rpc={rpc} onChanged={onChanged} />}

      <FactsGrid p={person} t={t} />

      {person.relationships && person.relationships.length > 0 && (
        <div>
          <p className="text-micro font-bold text-text-muted uppercase mb-2 tracking-wider">
            {t('settings.personCatalog.listing.relationshipsTitlePrefix')} ({person.relationships.length})
          </p>
          <div className="grid grid-cols-2 gap-2">
            {person.relationships.map((rel) => (
              <RelationshipRow key={`${rel.type}:${rel.name}`} rel={rel} />
            ))}
          </div>
        </div>
      )}

      {person.awards?.length ? (
        <div>
          <p className="text-micro font-bold text-text-muted uppercase mb-1 tracking-wider">
            {t('settings.personCatalog.listing.awardsTitle')}
          </p>
          <div className="flex flex-wrap gap-1">
            {person.awards.map((a) => (
              <StatusPill key={a} variant="warn" label={a} />
            ))}
          </div>
        </div>
      ) : null}

      {person.tags.length > 0 && (
        <div>
          <p className="text-micro font-bold text-text-muted uppercase mb-1 tracking-wider">
            {t('settings.personCatalog.listing.tagsTitle')}
          </p>
          <div className="flex flex-wrap gap-1">
            {person.tags.map((tag) => (
              <StatusPill key={tag} variant="neutral" label={`#${tag}`} />
            ))}
          </div>
        </div>
      )}

      {person.attributes.length > 0 && (
        <div>
          <p className="text-micro font-bold text-text-muted uppercase mb-1 tracking-wider">
            {t('settings.personCatalog.listing.attributesTitlePrefix')} ({person.attributes.length})
          </p>
          <div className="grid grid-cols-2 gap-1">
            {person.attributes.map((attr) => (
              <div
                key={`${attr.key}:${attr.value}`}
                className="bg-surface-raise rounded-lg px-2.5 py-1 border border-border-subtle flex items-center justify-between"
              >
                <span className="text-caption text-text-muted">{attr.key}</span>
                <span className="text-caption text-text-main font-medium">{attr.value}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {person.mentions.length > 0 && (
        <div>
          <p className="text-micro font-bold text-text-muted uppercase mb-1 tracking-wider">
            {t('settings.personCatalog.listing.recentMentionsTitlePrefix')} ({person.mentions.length})
          </p>
          <div className="space-y-1">
            {[...person.mentions]
              .reverse()
              .slice(0, 10)
              .map((s) => (
                <div key={s.id} className="bg-surface-raise rounded-lg px-2.5 py-1.5 border border-border-subtle">
                  <div className="flex items-center gap-2">
                    <StatusPill variant="info" label={s.source} />
                    {s.appName && <StatusPill variant="ok" label={s.appName} />}
                    <span className="ml-auto text-caption text-text-subtle font-mono">
                      {fmtDate(s.timestamp, locale)}
                    </span>
                  </div>
                  {s.text && <p className="text-caption text-text-subtle mt-0.5 truncate">{s.text}</p>}
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  )
}
