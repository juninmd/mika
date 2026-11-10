import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { createUiLogger } from '../logger'
import { accentVars } from '../utils/accent'
import { FieldAdornment, FieldHeader, FieldNotes } from './AuroraFieldParts'
import {
  type AuroraFieldProps,
  buildInputClass,
  buildWrapperClass,
  INPUT_PADDING,
  parseNumberDraft,
  readInitial,
} from './aurora-field-helpers'

const log = createUiLogger('ui:aurora-field')

export function AuroraField(props: AuroraFieldProps) {
  const {
    label,
    description,
    configPath,
    onSave,
    type = 'text',
    placeholder,
    badge,
    accentHue: accentHueProp,
    rows = 3,
    disabled = false,
    warning,
    onChange,
    onSubmit,
    trailing,
    hideLabel,
    onCommit,
    describedBy,
    invalid,
  } = props
  const id = useId()
  const controlled = onChange !== undefined
  const initial = readInitial(props)
  const [localDraft, setDraft] = useState(initial)
  const draft = controlled ? initial : localDraft
  const [focused, setFocused] = useState(false)
  const [saved, setSaved] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [showPw, setShowPw] = useState(false)
  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // O Escape cancela a edição e provoca um blur. Sem esta trava o `onBlur` chamaria `handleSave` com
  // o `draft`/`dirty` do render corrente e salvaria justamente o texto que o usuário cancelou.
  const skipBlurSaveRef = useRef(false)

  useEffect(() => {
    return () => {
      if (savedTimerRef.current) clearTimeout(savedTimerRef.current)
    }
  }, [])

  useEffect(() => setDraft(initial), [initial])

  const dirty = draft !== initial

  const handleSave = useCallback(async () => {
    if (!dirty || !onSave || !configPath) return

    // A blank/invalid number must not be coerced to 0 (it would overwrite e.g. the user's age): keep the draft, say why.
    const parsed = type === 'number' ? parseNumberDraft(draft) : draft
    if (parsed === null) {
      setSaveError(t('sdkUi.auroraField.invalidNumber'))
      return
    }
    try {
      await onSave(configPath, parsed)
    } catch (error) {
      const message = toErrorMessage(error)
      log.error('failed to save setting', { configPath, error: message })
      setSaveError(message)
      setDraft(initial)
      return
    }
    setSaveError(null)
    setSaved(true)
    if (savedTimerRef.current) clearTimeout(savedTimerRef.current)
    savedTimerRef.current = setTimeout(() => setSaved(false), 1200)
  }, [dirty, draft, type, configPath, onSave, initial])

  const commit = () => (controlled ? onCommit?.() : void handleSave())
  // Escape desfaz a edição; numa busca controlada "desfazer" é limpar o filtro. Devolve se havia o que
  // desfazer: só então o Escape é consumido — senão ele sobe e fecha a janela de settings.
  const cancel = (): boolean => {
    if (!controlled) {
      setDraft(initial)
      return dirty
    }
    if (type !== 'search' || draft === '') return false
    onChange('')
    return true
  }

  const inputClass = buildInputClass(focused)
  const sharedProps = {
    id,
    'data-sdk-control': 'field',
    'aria-describedby': [describedBy, warning && `${id}-warning`].filter(Boolean).join(' ') || undefined,
    'aria-invalid': invalid || undefined,
    // Chave de API revelada pelo olho não pode virar texto corrigido/autocompletado pelo sistema.
    autoComplete: type === 'password' ? 'off' : undefined,
    spellCheck: type === 'password' ? false : undefined,
    value: draft,
    placeholder,
    disabled,
    className: inputClass,

    // Border plus a thin soft ring: the old 20px halo and tinted fill read as a heavy purple frame.
    style: focused
      ? {
          boxShadow: '0 0 0 3px var(--accent-soft)',
          borderColor: 'color-mix(in srgb, var(--accent) 55%, transparent)',
        }
      : {},
    onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      if (controlled) {
        onChange(e.target.value)
        return
      }
      setSaveError(null)
      setDraft(e.target.value)
    },
    onFocus: () => setFocused(true),
    onBlur: () => {
      setFocused(false)
      if (skipBlurSaveRef.current) {
        skipBlurSaveRef.current = false
        return
      }
      commit()
    },
    onKeyDown: (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && type !== 'textarea') {
        if (onSubmit) onSubmit()
        else commit()
      } else if (e.key === 'Escape') {
        skipBlurSaveRef.current = true
        if (cancel()) e.preventDefault()
        ;(e.currentTarget as HTMLElement).blur()
      }
    },
  }

  return (
    <div className={buildWrapperClass(disabled)} style={accentVars(accentHueProp)}>
      <FieldHeader id={id} label={label} badge={badge} saved={saved} dirty={dirty} hideLabel={hideLabel} />

      <div className="flex items-stretch gap-2">
        <div className="relative min-w-0 flex-1">
          {type === 'textarea' ? (
            <textarea {...sharedProps} rows={rows} className={`${inputClass} resize-none`} />
          ) : (
            <input
              {...sharedProps}
              type={type === 'password' && showPw ? 'text' : type}
              className={`${inputClass} ${INPUT_PADDING[type] ?? ''}`}
            />
          )}
          <FieldAdornment type={type} showPw={showPw} onTogglePw={() => setShowPw((p) => !p)} />
        </div>
        {trailing}
      </div>

      <FieldNotes description={description} saveError={saveError} warning={warning} warningId={`${id}-warning`} />
    </div>
  )
}
