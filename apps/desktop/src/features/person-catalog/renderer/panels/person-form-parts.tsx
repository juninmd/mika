import { AuroraField, ConfirmButton } from '@mika/sdk-ui/settings'
import { Plus, X } from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import type { PersonRecord, PersonRelationship } from '../person-catalog.types'

// Pura, extraída de `PersonForm` — cada campo aqui é um `??` independente (não uma sequência do
// MESMO operador que o Biome soma como 1 só), e cada um soma na complexidade cognitiva do
// componente que os chama (AGENTS.md §3, teto 15). Mover o cálculo dos defaults pra fora é o que
// mantém `PersonForm` sob o teto depois que este pacote acrescentou `relationToUser`/`relationships`.
export function initialSaveFields(initial: Partial<PersonRecord> | undefined): SavePayloadFields {
  return {
    name: initial?.primaryName ?? '',
    type: initial?.type ?? 'real_person',
    bio: initial?.bio ?? '',
    role: initial?.role ?? '',
    relationToUser: initial?.relationToUser ?? '',
    relationships: initial?.relationships ?? [],
    origin: initial?.origin ?? '',
    nationality: initial?.nationality ?? '',
    imageUrl: initial?.imageUrl ?? '',
    userNotes: initial?.userNotes ?? '',
    tagsStr: (initial?.tags ?? []).join(', '),
  }
}

export interface SavePayloadFields {
  name: string
  type: string
  bio: string
  role: string
  relationToUser: string
  relationships: PersonRelationship[]
  origin: string
  nationality: string
  imageUrl: string
  userNotes: string
  tagsStr: string
}

// Pura, extraída de `PersonForm::handleSave` só para manter a complexidade cognitiva do componente
// sob o teto do Biome (<= 15, AGENTS.md §3) depois que este pacote acrescentou
// `relationToUser`/`relationships` ao payload de save.
export function buildSavePayload(fields: SavePayloadFields): Record<string, unknown> {
  return {
    name: fields.name.trim(),
    type: fields.type,
    bio: fields.bio.trim() || undefined,
    role: fields.role.trim() || undefined,
    relationToUser: fields.relationToUser.trim() || undefined,
    // Linha em branco (usuário clicou "add" e não preencheu) não vira aresta vazia no grafo —
    // `tool.rs::parse_relationships` não filtra nome/tipo vazios, quem filtra é aqui.
    relationships: fields.relationships
      .map((rel) => ({ ...rel, name: rel.name.trim(), type: rel.type.trim() }))
      .filter((rel) => rel.name && rel.type),
    origin: fields.origin.trim() || undefined,
    nationality: fields.nationality.trim() || undefined,
    imageUrl: fields.imageUrl.trim() || undefined,
    userNotes: fields.userNotes.trim() || undefined,
    tags: fields.tagsStr
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean),
  }
}

export const BTN_PRIMARY =
  'rounded-lg border border-accent-rim bg-accent-soft text-accent-text text-xs px-3 py-1.5 hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition font-bold disabled:opacity-50'
export const BTN_GHOST =
  'rounded-lg text-text-muted text-xs px-3 py-1.5 hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition'

export async function runWithLoading(
  setLoading: (loading: boolean) => void,
  operation: () => Promise<void>,
): Promise<void> {
  setLoading(true)
  try {
    await operation()
  } finally {
    setLoading(false)
  }
}

export function NameField({
  t,
  name,
  nameError,
  onChange,
}: {
  t: Translate
  name: string
  nameError: boolean
  onChange: (value: string) => void
}) {
  return (
    <AuroraField
      label={t('settings.personCatalog.form.nameLabel')}
      value={name}
      onChange={onChange}
      placeholder={t('settings.personCatalog.form.namePlaceholder')}
      warning={nameError ? t('settings.personCatalog.form.nameRequiredError') : undefined}
      invalid={nameError}
    />
  )
}

// Item 7 do pacote "people catalog + relationship graph": editor de arestas pessoa-pessoa
// (`PersonRecord.relationships`) — antes desta rodada só o detalhe (leitura) tinha UI, o form nunca
// deixava adicionar/remover uma. Sem id estável nas linhas (o índice do array já basta: só
// add/remove, nunca reordena).
export function RelationshipsEditor({
  t,
  relationships,
  onChange,
}: {
  t: Translate
  relationships: PersonRelationship[]
  onChange: (value: PersonRelationship[]) => void
}) {
  function updateRow(index: number, patch: Partial<PersonRelationship>) {
    onChange(relationships.map((rel, i) => (i === index ? { ...rel, ...patch } : rel)))
  }
  function removeRow(index: number) {
    onChange(relationships.filter((_, i) => i !== index))
  }
  function addRow() {
    onChange([...relationships, { name: '', type: '' }])
  }

  return (
    <div className="flex flex-col gap-2">
      <p className="text-micro font-bold text-text-muted uppercase tracking-wider">
        {t('settings.personCatalog.form.relationshipsLabel')}
      </p>
      {relationships.map((rel, index) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: linhas sem id estável, só add/remove no fim/posição
        <div key={index} className="flex items-center gap-2">
          <AuroraField
            label={t('settings.personCatalog.form.relationshipNamePlaceholder')}
            hideLabel
            value={rel.name}
            onChange={(value) => updateRow(index, { name: value })}
            placeholder={t('settings.personCatalog.form.relationshipNamePlaceholder')}
          />
          <AuroraField
            label={t('settings.personCatalog.form.relationshipTypePlaceholder')}
            hideLabel
            value={rel.type}
            onChange={(value) => updateRow(index, { type: value })}
            placeholder={t('settings.personCatalog.form.relationshipTypePlaceholder')}
          />
          <button
            type="button"
            onClick={() => removeRow(index)}
            aria-label={t('settings.personCatalog.form.removeRelationshipAriaLabel')}
            className="rounded-lg text-text-muted hover:text-danger p-1.5 shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus transition-colors"
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </div>
      ))}
      <button type="button" onClick={addRow} className={`${BTN_GHOST} self-start flex items-center gap-1`}>
        <Plus aria-hidden="true" className="size-3.5" />
        {t('settings.personCatalog.form.addRelationshipButton')}
      </button>
    </div>
  )
}

export function DangerActions({
  t,
  personName,
  onDelete,
  onBlock,
  deleting,
  blocking,
}: {
  t: Translate
  personName: string
  onDelete?: () => void
  onBlock?: () => void
  deleting: boolean
  blocking: boolean
}) {
  return (
    <>
      {onDelete && (
        <div className="ml-auto">
          <ConfirmButton
            ariaLabel={`${t('settings.personCatalog.card.deleteAriaLabelPrefix')} "${personName}" ${t('settings.personCatalog.card.deleteAriaLabelSuffix')}`}
            label={deleting ? t('settings.personCatalog.form.deletingLabel') : t('button.delete')}
            disabled={deleting}
            onConfirm={onDelete}
          />
        </div>
      )}
      {onBlock && (
        <ConfirmButton
          ariaLabel={`${t('settings.personCatalog.form.blockAriaLabelPrefix')} "${personName}" ${t('settings.personCatalog.form.blockAriaLabelSuffix')}`}
          label={
            blocking
              ? t('settings.personCatalog.form.blockingLabel')
              : t('settings.personCatalog.form.blockButtonLabel')
          }
          disabled={blocking}
          onConfirm={onBlock}
        />
      )}
    </>
  )
}
