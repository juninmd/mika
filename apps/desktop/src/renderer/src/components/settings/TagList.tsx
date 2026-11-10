import { AuroraField } from '@mika/sdk-ui/settings'
import { Plus, X } from 'lucide-react'
import { useState } from 'react'

type TagListProps = {
  label: string
  description?: string
  tags: string[]
  onChange: (tags: string[]) => void
  placeholder?: string
  maxTags?: number

  addLabel?: string
  removeLabel?: string
}

export function TagList({
  label,
  description,
  tags,
  onChange,
  placeholder = 'Add item...',
  maxTags,
  addLabel = 'Add',
  removeLabel = 'Remove',
}: TagListProps) {
  const [input, setInput] = useState('')

  const addTag = () => {
    const trimmed = input.trim()
    if (!trimmed || (maxTags && tags.length >= maxTags) || tags.includes(trimmed)) {
      setInput('')
      return
    }
    onChange([...tags, trimmed])
    setInput('')
  }

  const removeTag = (tag: string) => {
    onChange(tags.filter((t) => t !== tag))
  }

  return (
    <div className="space-y-3">
      <div>
        <p className="text-caption font-semibold text-text-main">{label}</p>
        {description && <p className="mt-0.5 text-caption text-text-muted">{description}</p>}
      </div>

      <div className="flex flex-wrap gap-2">
        {tags.map((tag) => (
          <span
            key={tag}
            // Theme tokens, not a hue of its own: the raw hsl() ignored the theme and rendered teal.
            className="group inline-flex max-w-[16rem] items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 py-1.5 text-caption font-medium text-accent-text transition-colors"
          >
            <span className="truncate" title={tag}>
              {tag}
            </span>
            <button
              type="button"
              onClick={() => removeTag(tag)}
              className="-mr-1.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md opacity-70 transition-colors hover:bg-surface-raise-strong hover:text-danger hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              aria-label={`${removeLabel} ${tag}`}
              title={`${removeLabel} ${tag}`}
            >
              <X size={12} aria-hidden="true" />
            </button>
          </span>
        ))}
      </div>

      <AuroraField
        label={label}
        hideLabel
        value={input}
        onChange={setInput}
        onSubmit={addTag}
        placeholder={placeholder}
        trailing={
          <button
            type="button"
            onClick={addTag}
            disabled={maxTags ? tags.length >= maxTags : false}
            className="inline-flex items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 py-2 text-caption font-semibold text-accent-text transition-colors hover:bg-accent/25 disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            aria-label={`${addLabel}: ${label}`}
          >
            <Plus size={14} aria-hidden="true" />
            {addLabel}
          </button>
        }
      />

      {maxTags && (
        <p className="text-caption font-medium text-text-muted">
          {tags.length}/{maxTags} items
        </p>
      )}
    </div>
  )
}
