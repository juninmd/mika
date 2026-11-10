import { AuroraField, GlassSelect } from '@mika/sdk-ui/settings'
import { useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { PERSON_TYPE_VALUES } from '../../../../shared/person-types'
import { typeLabel } from '../components/person-card'
import type { PersonRecord, PersonRelationship } from '../person-catalog.types'
import {
  BTN_GHOST,
  BTN_PRIMARY,
  buildSavePayload,
  DangerActions,
  initialSaveFields,
  NameField,
  RelationshipsEditor,
  runWithLoading,
} from './person-form-parts'

interface Props {
  initial?: Partial<PersonRecord>
  onSave: (data: Record<string, unknown>) => Promise<void>
  onDelete?: () => Promise<void>

  onBlock?: () => Promise<void>
  onCancel: () => void
}

export function PersonForm({ initial, onSave, onDelete, onBlock, onCancel }: Props) {
  const { t } = useI18n()
  const defaults = initialSaveFields(initial)
  const [name, setName] = useState(defaults.name)
  const [type, setType] = useState(defaults.type)
  const [bio, setBio] = useState(defaults.bio)
  const [role, setRole] = useState(defaults.role)
  const [relationToUser, setRelationToUser] = useState(defaults.relationToUser)
  const [relationships, setRelationships] = useState<PersonRelationship[]>(defaults.relationships)
  const [origin, setOrigin] = useState(defaults.origin)
  const [nationality, setNationality] = useState(defaults.nationality)
  const [imageUrl, setImageUrl] = useState(defaults.imageUrl)
  const [userNotes, setUserNotes] = useState(defaults.userNotes)
  const [tagsStr, setTagsStr] = useState(defaults.tagsStr)
  const [saving, setSaving] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [blocking, setBlocking] = useState(false)
  const [nameError, setNameError] = useState(false)
  const personName = initial?.primaryName ?? name

  async function handleSave() {
    if (!name.trim()) {
      setNameError(true)
      return
    }
    const payload = buildSavePayload({
      name,
      type,
      bio,
      role,
      relationToUser,
      relationships,
      origin,
      nationality,
      imageUrl,
      userNotes,
      tagsStr,
    })
    await runWithLoading(setSaving, () => onSave(payload))
  }

  async function handleDelete() {
    if (!onDelete) return
    await runWithLoading(setDeleting, onDelete)
  }

  async function handleBlock() {
    if (!onBlock) return
    await runWithLoading(setBlocking, onBlock)
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-2 gap-3">
        <NameField
          t={t}
          name={name}
          nameError={nameError}
          onChange={(value) => {
            setName(value)
            if (nameError) setNameError(false)
          }}
        />
        <GlassSelect
          label={t('settings.personCatalog.form.typeLabel')}
          value={type}
          onChange={(v) => setType(v as typeof type)}
          options={PERSON_TYPE_VALUES.map((v) => ({ label: typeLabel(t, v), value: v }))}
        />
        <AuroraField
          label={t('settings.personCatalog.form.roleLabel')}
          value={role}
          onChange={setRole}
          placeholder={t('settings.personCatalog.form.rolePlaceholder')}
        />
        <AuroraField
          label={t('settings.personCatalog.form.relationToUserLabel')}
          value={relationToUser}
          onChange={setRelationToUser}
          placeholder={t('settings.personCatalog.form.relationToUserPlaceholder')}
        />
        <AuroraField
          label={t('settings.personCatalog.form.originLabel')}
          value={origin}
          onChange={setOrigin}
          placeholder={t('settings.personCatalog.form.originPlaceholder')}
        />
        <AuroraField
          label={t('settings.personCatalog.form.nationalityLabel')}
          value={nationality}
          onChange={setNationality}
          placeholder={t('settings.personCatalog.form.nationalityPlaceholder')}
        />
        <AuroraField
          label={t('settings.personCatalog.form.imageUrlLabel')}
          value={imageUrl}
          onChange={setImageUrl}
          type="url"
          placeholder="https://..."
        />
      </div>
      <AuroraField
        label={t('settings.personCatalog.form.bioLabel')}
        value={bio}
        onChange={setBio}
        type="textarea"
        rows={3}
        placeholder={t('settings.personCatalog.form.bioPlaceholder')}
      />
      <AuroraField
        label={t('settings.personCatalog.form.notesLabel')}
        value={userNotes}
        onChange={setUserNotes}
        placeholder={t('settings.personCatalog.form.notesPlaceholder')}
      />
      <AuroraField
        label={t('settings.personCatalog.form.tagsLabel')}
        value={tagsStr}
        onChange={setTagsStr}
        placeholder={t('settings.personCatalog.form.tagsPlaceholder')}
      />
      <RelationshipsEditor t={t} relationships={relationships} onChange={setRelationships} />
      <div className="flex items-center gap-2 pt-1">
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={saving || !name.trim()}
          className={BTN_PRIMARY}
        >
          {saving ? t('settings.personCatalog.form.savingLabel') : t('button.save')}
        </button>
        <button type="button" onClick={onCancel} className={BTN_GHOST}>
          {t('button.cancel')}
        </button>
        <DangerActions
          t={t}
          personName={personName}
          onDelete={onDelete ? () => void handleDelete() : undefined}
          onBlock={onBlock ? () => void handleBlock() : undefined}
          deleting={deleting}
          blocking={blocking}
        />
      </div>
    </div>
  )
}
