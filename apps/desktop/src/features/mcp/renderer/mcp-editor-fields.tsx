import { AuroraField, MOTION, SectionLabel } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { CheckCircle2, ExternalLink, KeyRound, ListOrdered, Loader2, Plus, TriangleAlert, X } from 'lucide-react'
import { useState } from 'react'
import type { useI18n } from '../../../renderer/src/hooks/useI18n'
import { openExternalLink } from '../../../renderer/src/lib/shell-links'
import { type EnvRow, errorHint, type McpFormErrorCode, type McpFormErrors, splitCommandLine } from './mcp-server-form'
import { BTN } from './mcp-ui'

type T = ReturnType<typeof useI18n>['t']

export const errorText = (t: T, code: McpFormErrorCode | undefined): string | undefined =>
  code ? t(`settings.mcp.form.error.${code}`) : undefined

type ArgsEditorProps = {
  args: string[]
  onChange: (args: string[]) => void
  placeholder: string
  error?: string
  t: T
}

/** Args em ordem (repetição permitida: `--flag x --flag y`); colar uma linha inteira separa em vários. */
export function ArgsEditor({ args, onChange, placeholder, error, t }: ArgsEditorProps) {
  const [draft, setDraft] = useState('')
  const add = () => {
    const parts = splitCommandLine(draft)
    if (parts.length > 0) onChange([...args, ...parts])
    setDraft('')
  }
  return (
    <div className="space-y-2.5">
      <SectionLabel icon={ListOrdered} count={args.length}>
        {t('settings.mcp.form.args')}
      </SectionLabel>
      {args.length > 0 && (
        <ol className="flex flex-wrap gap-1.5">
          <AnimatePresence initial={false}>
            {args.map((arg, i) => (
              <motion.li
                // biome-ignore lint/suspicious/noArrayIndexKey: args repetem e a posição é a identidade
                key={`${i}-${arg}`}
                {...MOTION.popover}
                className="group inline-flex max-w-full items-center gap-1.5 rounded-lg border border-border-subtle bg-bg-deep/60 py-1 pl-2 pr-1"
              >
                <span className="text-micro tabular-nums text-text-subtle">{i + 1}</span>
                <code className="truncate font-mono text-caption text-text-main" title={arg}>
                  {arg}
                </code>
                <button
                  type="button"
                  onClick={() => onChange(args.filter((_, j) => j !== i))}
                  aria-label={t('settings.mcp.form.removeArg').replace('{{arg}}', arg)}
                  className="flex h-5 w-5 items-center justify-center rounded text-text-subtle transition-colors hover:bg-danger-soft hover:text-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                >
                  <X size={11} aria-hidden="true" />
                </button>
              </motion.li>
            ))}
          </AnimatePresence>
        </ol>
      )}
      <AuroraField
        label={t('settings.mcp.form.addArg')}
        hideLabel
        value={draft}
        onChange={setDraft}
        onSubmit={add}
        onCommit={add}
        placeholder={placeholder}
        warning={error}
        invalid={Boolean(error)}
        trailing={
          <button type="button" className={BTN.ghost} onClick={add} disabled={!draft.trim()}>
            <Plus size={13} aria-hidden="true" />
            {t('settings.mcp.form.addArg')}
          </button>
        }
      />
    </div>
  )
}

type EnvEditorProps = {
  rows: EnvRow[]
  onChange: (rows: EnvRow[]) => void
  errors: McpFormErrors
  keyUrl?: string
  t: T
}

let rowSeq = 0

export function EnvEditor({ rows, onChange, errors, keyUrl, t }: EnvEditorProps) {
  const patch = (id: string, change: Partial<EnvRow>) =>
    onChange(rows.map((row) => (row.id === id ? { ...row, ...change } : row)))
  const addRow = () => {
    rowSeq += 1
    onChange([...rows, { id: `new-${rowSeq}`, key: '', value: '', saved: false }])
  }
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between gap-2">
        <SectionLabel icon={KeyRound} count={rows.length}>
          {t('settings.mcp.form.env')}
        </SectionLabel>
        <div className="flex items-center gap-2">
          {keyUrl && (
            <button
              type="button"
              className={BTN.subtle}
              onClick={() => void openExternalLink(keyUrl, t('settings.providerCredentials.getKeyError'))}
            >
              <ExternalLink size={12} aria-hidden="true" />
              {t('settings.providerCredentials.getKey')}
            </button>
          )}
          <button type="button" className={BTN.subtle} onClick={addRow}>
            <Plus size={12} aria-hidden="true" />
            {t('settings.mcp.form.addEnv')}
          </button>
        </div>
      </div>
      {rows.length === 0 && <p className="text-caption text-text-subtle">{t('settings.mcp.form.envEmpty')}</p>}
      <AnimatePresence initial={false}>
        {rows.map((row) => (
          <motion.div
            key={row.id}
            {...MOTION.fadeUp}
            className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-start gap-2"
          >
            <AuroraField
              label={t('settings.mcp.form.envKey')}
              hideLabel
              value={row.key}
              onChange={(key) => patch(row.id, { key: key.toUpperCase().replace(/\s/g, '_'), saved: false })}
              placeholder={t('settings.mcp.form.envKeyPlaceholder')}
              warning={errorText(t, errors[`env.${row.id}`])}
              invalid={Boolean(errors[`env.${row.id}`])}
            />
            <AuroraField
              label={t('settings.mcp.form.envValue').replace('{{key}}', row.key)}
              hideLabel
              type="password"
              value={row.value}
              onChange={(value) => patch(row.id, { value })}
              placeholder={row.saved ? t('settings.mcp.form.secretSaved') : t('settings.mcp.form.envValuePlaceholder')}
            />
            <button
              type="button"
              className={`${BTN.icon} mt-0.5`}
              onClick={() => onChange(rows.filter((r) => r.id !== row.id))}
              aria-label={t('settings.mcp.form.removeEnv').replace('{{key}}', row.key || '—')}
            >
              <X size={14} aria-hidden="true" />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  )
}

export type TestState =
  | { status: 'idle' }
  | { status: 'testing' }
  | { status: 'ok'; tools: { name: string; description?: string | null }[] }
  | { status: 'error'; error: string }

export function TestResult({ state, command, t }: { state: TestState; command: string; t: T }) {
  return (
    <AnimatePresence initial={false}>
      {state.status === 'testing' && (
        <motion.div
          key="testing"
          {...MOTION.fadeUp}
          role="status"
          className="flex items-center gap-2 text-caption text-text-muted"
        >
          <Loader2 size={14} className="animate-spin text-accent" aria-hidden="true" />
          {t('settings.mcp.test.running')}
        </motion.div>
      )}
      {state.status === 'ok' && (
        <motion.div
          key="ok"
          {...MOTION.fadeUp}
          role="status"
          className="space-y-2 rounded-xl border border-ok-rim bg-ok-soft p-3"
        >
          <p className="flex items-center gap-2 text-caption font-semibold text-ok">
            <CheckCircle2 size={14} aria-hidden="true" />
            {t('settings.mcp.test.ok', { count: state.tools.length })}
          </p>
          <ul className="flex max-h-28 flex-wrap gap-1.5 overflow-y-auto">
            {state.tools.map((tool) => (
              <li key={tool.name} title={tool.description ?? undefined}>
                <code className="rounded-md border border-ok-rim bg-bg-deep/40 px-1.5 py-0.5 font-mono text-caption text-ok">
                  {tool.name}
                </code>
              </li>
            ))}
          </ul>
        </motion.div>
      )}
      {state.status === 'error' && (
        <motion.div
          key="error"
          {...MOTION.fadeUp}
          role="alert"
          className="space-y-1 rounded-xl border border-danger-rim bg-danger-soft p-3"
        >
          <p className="flex items-center gap-2 text-caption font-semibold text-danger">
            <TriangleAlert size={14} aria-hidden="true" />
            {t('settings.mcp.test.failed')}
          </p>
          <p className="break-words font-mono text-caption text-danger">{state.error}</p>
          <p className="text-caption leading-relaxed text-text-muted">
            {t(`settings.mcp.hint.${errorHint(state.error, command)}`)}
          </p>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
