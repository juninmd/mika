import { t } from '@mika/sdk/i18n'
import { AnimatePresence, motion } from 'framer-motion'
import { Check, ChevronDown } from 'lucide-react'
import { createPortal } from 'react-dom'
import { DURATION, EASE, MOTION } from '../motion'
import { accentVars } from '../utils/accent'
import { cn } from '../utils/cn'
import { GlassChoiceCards } from './GlassChoiceCards'
import {
  containerClass,
  type GlassSelectOption,
  type GlassSelectProps,
  LIST_MAX_HEIGHT,
  rendersAsCards,
  triggerClass,
} from './glass-select-helpers'
import { useGlassSelect } from './use-glass-select'

export function GlassSelect(props: GlassSelectProps) {
  const { label, description, searchable, accentHue: accentHueProp, disabled = false, hideLabel, describedBy } = props
  const {
    accentHue,
    selectId,
    listboxId,
    isOpen,
    search,
    setSearch,
    activeIndex,
    setActiveIndex,
    saveError,
    position,
    triggerRef,
    panelRef,
    focusOnActiveChangeRef,
    currentValue,
    selectedOption,
    filteredOptions,
    close,
    open,
    selectOption,
    handlePanelKeyDown,
    activeDescendant,
  } = useGlassSelect(props)

  if (rendersAsCards(props)) {
    return (
      <GlassSelectCards
        props={props}
        selectId={selectId}
        value={currentValue}
        saveError={saveError}
        onSelect={selectOption}
      />
    )
  }

  return (
    <div className={containerClass(disabled)} style={accentVars(accentHueProp)}>
      <label
        id={`${selectId}-label`}
        htmlFor={selectId}
        className={hideLabel ? 'sr-only' : 'text-[13px] font-medium text-text-main'}
      >
        {label}
      </label>

      <button
        ref={triggerRef}
        data-sdk-control="select"
        id={selectId}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-describedby={describedBy}
        aria-expanded={isOpen}
        aria-controls={isOpen ? listboxId : undefined}
        aria-labelledby={`${selectId}-label ${selectId}`}
        onClick={() => (isOpen ? close(false) : open())}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            open()
          } else if (event.key === 'Escape' && isOpen) {
            event.preventDefault()
            close(true)
          }
        }}
        className={triggerClass(isOpen)}
      >
        <div className="flex items-center gap-2 truncate">
          {selectedOption?.icon}
          <span className="truncate" title={selectedOption?.label}>
            {selectedOption?.label ?? t('sdkUi.select.placeholder')}
          </span>
        </div>
        <motion.div animate={{ rotate: isOpen ? 180 : 0 }} transition={{ duration: DURATION.fast, ease: EASE.out }}>
          <ChevronDown size={16} aria-hidden="true" className="text-text-muted" />
        </motion.div>
      </button>

      {createPortal(
        <AnimatePresence>
          {isOpen && (
            <motion.div
              ref={panelRef}
              tabIndex={-1}
              onKeyDown={handlePanelKeyDown}
              {...MOTION.popover}
              className="fixed max-h-[calc(100vh-16px)] flex flex-col overflow-hidden rounded-xl border border-border-subtle bg-bg-surface/95 outline-none backdrop-blur-2xl shadow-[0_20px_60px_rgba(0,0,0,0.7)]"
              style={
                {
                  top: position?.top ?? 0,
                  left: position?.left ?? 0,
                  width: position?.width,

                  zIndex: 'var(--layer-dropdown, 100)',
                  transformOrigin: 'top center',

                  ...accentVars(accentHue),
                } as React.CSSProperties
              }
            >
              {searchable && (
                <div className="p-2 border-b border-border-subtle">
                  <input
                    data-sdk-control="select-search"
                    type="text"
                    role="combobox"
                    placeholder={t('sdkUi.select.searchPlaceholder')}
                    aria-label={t('sdkUi.select.searchAriaLabel')}
                    aria-expanded
                    aria-controls={listboxId}
                    aria-autocomplete="list"
                    aria-activedescendant={activeDescendant}
                    value={search}
                    onChange={(e) => {
                      setSearch(e.target.value)
                      setActiveIndex(0)
                    }}
                    className="w-full rounded border border-border-subtle bg-surface-raise px-2 py-1.5 text-xs text-text-main outline-none focus:border-accent-rim"
                  />
                </div>
              )}
              <div
                id={listboxId}
                role="listbox"
                aria-label={label}
                className="overflow-y-auto p-1"
                style={{ maxHeight: position?.listMaxHeight ?? LIST_MAX_HEIGHT }}
              >
                {filteredOptions.length === 0 ? (
                  <div className="p-2 text-center text-xs text-text-muted">{t('sdkUi.select.noOptionsFound')}</div>
                ) : (
                  filteredOptions.map((opt, index) => (
                    <SelectOptionRow
                      key={opt.value}
                      option={opt}
                      id={`${listboxId}-opt-${index}`}
                      active={index === activeIndex}
                      selected={currentValue === opt.value}
                      onHover={() => {
                        focusOnActiveChangeRef.current = false
                        setActiveIndex(index)
                      }}
                      onPick={() => selectOption(opt.value)}
                    />
                  ))
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}

      {saveError && (
        <p role="alert" className="text-caption leading-snug text-danger">
          {saveError || t('sdkUi.saveError')}
        </p>
      )}

      {description && <p className="text-caption leading-snug text-text-muted">{description}</p>}
    </div>
  )
}

function GlassSelectCards({
  props,
  selectId,
  value,
  saveError,
  onSelect,
}: {
  props: GlassSelectProps
  selectId: string
  value: string
  saveError: string | null | undefined
  onSelect: (value: string) => void
}) {
  const { label, description, describedBy, disabled = false, accentHue } = props
  const descriptionId = description ? `${selectId}-description` : undefined
  return (
    <div className={containerClass(disabled)} style={accentVars(accentHue)}>
      <div className="flex flex-col gap-0.5">
        <span id={`${selectId}-label`} className="text-[13px] font-medium text-text-main">
          {label}
        </span>
        {description && (
          <p id={descriptionId} className="text-caption leading-snug text-text-muted">
            {description}
          </p>
        )}
      </div>
      <GlassChoiceCards
        id={selectId}
        label={label}
        labelledBy={`${selectId}-label`}
        describedBy={[descriptionId, describedBy].filter(Boolean).join(' ') || undefined}
        options={props.options}
        value={value}
        disabled={disabled}
        onSelect={onSelect}
      />
      {saveError && (
        <p role="alert" className="text-caption leading-snug text-danger">
          {saveError}
        </p>
      )}
    </div>
  )
}

function SelectOptionRow({
  option,
  id,
  active,
  selected,
  onHover,
  onPick,
}: {
  option: GlassSelectOption
  id: string
  active: boolean
  selected: boolean
  onHover: () => void
  onPick: () => void
}) {
  return (
    <button
      id={id}
      type="button"
      role="option"
      aria-selected={selected}
      data-active={active}
      tabIndex={active ? 0 : -1}
      onMouseEnter={onHover}
      onClick={onPick}
      className={cn(
        'flex w-full items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-sm outline-none transition-colors hover:bg-surface-raise-strong',
        active && 'bg-surface-raise-strong',
        selected && 'bg-accent-soft text-accent font-medium border-l-2 border-accent',
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        {option.icon}
        <span className="min-w-0">
          <span className="block truncate" title={option.label}>
            {option.label}
          </span>
          {option.description && (
            <span className="block truncate text-caption font-normal text-text-muted" title={option.description}>
              {option.description}
            </span>
          )}
        </span>
      </div>
      {selected && <Check size={14} aria-hidden="true" className="shrink-0" />}
    </button>
  )
}
