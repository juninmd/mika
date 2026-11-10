import { toErrorMessage } from '@mika/sdk/utils/object'
import { AuroraField, DURATION, GlassToggle, MOTION, StatusPill } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { ExternalLink, FlaskConical, Lightbulb, Loader2, Save, Server, SquareTerminal, X } from 'lucide-react'
import { type KeyboardEvent, useEffect, useId, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { trapTab } from '../../../renderer/src/components/tool-approval-focus'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { openExternalLink } from '../../../renderer/src/lib/shell-links'
import { ArgsEditor, EnvEditor, errorText, TestResult, type TestState } from './mcp-editor-fields'
import { commandPreview, type McpServerForm, pasteCommandLine, specFromForm, validateForm } from './mcp-server-form'
import type { McpTemplate } from './mcp-templates'
import { BTN } from './mcp-ui'

export type EditorTarget = { form: McpServerForm; originalName?: string; template?: McpTemplate }

type EditorProps = {
  target: EditorTarget
  takenNames: string[]
  onClose: () => void
}

/** Colar `npx -y pacote` no campo de comando separa em comando + args; digitar não. */
const isPaste = (next: string, prev: string): boolean => next.length - prev.length > 1 && /\s/.test(next.trim())

function useEditor({ target, takenNames, onClose }: EditorProps) {
  const [form, setForm] = useState(target.form)
  const [showErrors, setShowErrors] = useState(false)
  const [test, setTest] = useState<TestState>({ status: 'idle' })
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const errors = useMemo(() => validateForm(form, takenNames, target.originalName), [form, takenNames, target])
  const valid = Object.keys(errors).length === 0

  const update = (patch: Partial<McpServerForm>) => {
    setForm((f) => ({ ...f, ...patch }))
    setTest({ status: 'idle' })
  }
  const setCommand = (command: string) =>
    isPaste(command, form.command) ? update(pasteCommandLine(form, command)) : update({ command })

  const runTest = async () => {
    setShowErrors(true)
    if (errors.command || errors.cwd || errors.args) return
    setTest({ status: 'testing' })
    try {
      const result = await getElectronApi().runtime.testMcpServer({
        spec: specFromForm(form),
        originalName: target.originalName,
      })
      setTest(result.ok ? { status: 'ok', tools: result.tools } : { status: 'error', error: result.error })
    } catch (error) {
      setTest({ status: 'error', error: toErrorMessage(error) })
    }
  }

  const save = async () => {
    setShowErrors(true)
    if (!valid) return
    setSaving(true)
    setSaveError(null)
    try {
      const result = await getElectronApi().runtime.saveMcpServer({
        name: form.name.trim(),
        spec: specFromForm(form),
        originalName: target.originalName,
      })
      if (result.ok) onClose()
      else setSaveError(result.error)
    } catch (error) {
      setSaveError(toErrorMessage(error))
    } finally {
      setSaving(false)
    }
  }

  const shown = showErrors ? errors : {}
  const blocked = showErrors && !valid
  return { form, update, setCommand, errors: shown, blocked, test, runTest, save, saving, saveError }
}

function NeedBanner({ template }: { template: McpTemplate }) {
  const { t } = useI18n()
  if (template.need === 'none') return null
  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-info-rim bg-info-soft px-3.5 py-2.5 text-caption leading-relaxed text-info">
      <Lightbulb size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
      <span className="min-w-0 flex-1">{t(`settings.mcp.template.needHelp.${template.need}`)}</span>
      {template.need !== 'apiKey' && (
        <button
          type="button"
          className="inline-flex shrink-0 items-center gap-1 rounded font-semibold hover:underline focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
          onClick={() => void openExternalLink(template.docsUrl, t('settings.providerCredentials.getKeyError'))}
        >
          <ExternalLink size={12} aria-hidden="true" />
          {t('settings.mcp.template.openDocs')}
        </button>
      )}
    </div>
  )
}

function EditorHeader({ target, titleId, onClose }: { target: EditorTarget; titleId: string; onClose: () => void }) {
  const { t } = useI18n()
  const title = target.originalName
    ? t('settings.mcp.editor.editTitle').replace('{{name}}', target.originalName)
    : t('settings.mcp.editor.newTitle')
  return (
    <header className="flex items-start gap-3 border-b border-border-subtle px-6 py-5">
      <div
        aria-hidden="true"
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent shadow-[0_0_18px_var(--accent-glow)]"
      >
        <Server size={18} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <h2 id={titleId} className="text-base font-bold text-text-main">
            {title}
          </h2>
          {target.template && <StatusPill variant="accent" label={target.template.brand} />}
        </div>
        <p className="mt-0.5 text-caption text-text-muted">{t('settings.mcp.editor.subtitle')}</p>
      </div>
      <button type="button" className={BTN.icon} onClick={onClose} aria-label={t('button.close')}>
        <X size={15} aria-hidden="true" />
      </button>
    </header>
  )
}

export function McpServerEditor(props: EditorProps) {
  const { t } = useI18n()
  const { target, onClose } = props
  const ed = useEditor(props)
  const titleId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)
  const busy = ed.saving || ed.test.status === 'testing'
  const resultRef = useRef<HTMLDivElement>(null)

  // O resultado do teste nasce no fim do corpo rolável: traga-o à vista em vez de deixá-lo abaixo da dobra.
  useEffect(() => {
    if (ed.test.status === 'idle') return
    const frame = requestAnimationFrame(() => resultRef.current?.scrollIntoView({ block: 'end', behavior: 'smooth' }))
    return () => cancelAnimationFrame(frame)
  }, [ed.test.status])

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null
    dialogRef.current?.querySelector<HTMLElement>('input')?.focus()
    return () => previous?.focus?.()
  }, [])

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Tab') trapTab(dialogRef.current, e)
    if (e.key === 'Escape' && !e.defaultPrevented) {
      // Swallowed even while busy: letting it bubble to the settings window's own Escape handler closed the whole window mid-test/save.
      e.stopPropagation()
      if (!busy) onClose()
    }
  }

  const preview = commandPreview(ed.form.command.trim() || '…', ed.form.args)

  return createPortal(
    <motion.div
      className="fixed inset-0 flex items-center justify-center p-4"
      style={{ zIndex: 'var(--layer-modal, 80)' }}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, transition: { duration: DURATION.fast } }}
    >
      <button
        type="button"
        tabIndex={-1}
        aria-label={t('button.close')}
        className="absolute inset-0 cursor-default bg-bg-deep/70 backdrop-blur-md"
        onClick={busy ? undefined : onClose}
      />
      <motion.div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
        {...MOTION.dialog}
        className="relative flex max-h-[calc(100vh-2rem)] w-[min(46rem,calc(100vw-2rem))] flex-col overflow-hidden rounded-2xl border border-border-strong bg-bg-surface shadow-2xl"
      >
        <EditorHeader target={target} titleId={titleId} onClose={onClose} />
        <div className="flex-1 space-y-5 overflow-y-auto px-6 py-5">
          {target.template && <NeedBanner template={target.template} />}
          <div className="grid items-start gap-4 sm:grid-cols-[minmax(0,1fr)_auto]">
            <AuroraField
              label={t('settings.mcp.form.name')}
              description={t('settings.mcp.form.nameHint')}
              value={ed.form.name}
              onChange={(name) => ed.update({ name })}
              placeholder={t('settings.mcp.form.namePlaceholder')}
              warning={errorText(t, ed.errors.name)}
              invalid={Boolean(ed.errors.name)}
            />
            <div className="pt-6 sm:w-48">
              <GlassToggle
                label={t('settings.mcp.form.enabled')}
                value={ed.form.enabled}
                onChange={(enabled) => ed.update({ enabled })}
              />
            </div>
          </div>
          <AuroraField
            label={t('settings.mcp.form.command')}
            description={t('settings.mcp.form.commandHint')}
            value={ed.form.command}
            onChange={ed.setCommand}
            placeholder={t('settings.mcp.form.commandPlaceholder')}
            warning={errorText(t, ed.errors.command)}
            invalid={Boolean(ed.errors.command)}
          />
          <ArgsEditor
            args={ed.form.args}
            onChange={(args) => ed.update({ args })}
            placeholder={
              target.template?.need === 'folder'
                ? t('settings.mcp.form.folderPlaceholder')
                : t('settings.mcp.form.argPlaceholder')
            }
            error={errorText(t, ed.errors.args)}
            t={t}
          />
          <EnvEditor
            rows={ed.form.env}
            onChange={(env) => ed.update({ env })}
            errors={ed.errors}
            keyUrl={target.template?.need === 'apiKey' ? target.template.docsUrl : undefined}
            t={t}
          />
          <AuroraField
            label={t('settings.mcp.form.cwd')}
            description={t('settings.mcp.form.cwdHint')}
            badge={t('settings.mcp.form.optional')}
            value={ed.form.cwd}
            onChange={(cwd) => ed.update({ cwd })}
            placeholder={t('settings.mcp.form.cwdPlaceholder')}
            warning={errorText(t, ed.errors.cwd)}
            invalid={Boolean(ed.errors.cwd)}
          />
          <div className="rounded-xl border border-border-subtle bg-bg-deep/60 px-3.5 py-2.5">
            <p className="mb-1 flex items-center gap-1.5 text-micro font-bold uppercase tracking-[0.12em] text-text-subtle">
              <SquareTerminal size={12} aria-hidden="true" />
              {t('settings.mcp.form.preview')}
            </p>
            <code className="block break-all font-mono text-caption text-text-main">{preview}</code>
          </div>
          <div ref={resultRef}>
            <TestResult state={ed.test} command={ed.form.command} t={t} />
          </div>
        </div>
        <footer className="flex flex-wrap items-center gap-2 border-t border-border-subtle bg-bg-deep/40 px-6 py-4">
          <button type="button" className={BTN.ghost} onClick={() => void ed.runTest()} disabled={busy}>
            {ed.test.status === 'testing' ? (
              <Loader2 size={13} className="animate-spin" aria-hidden="true" />
            ) : (
              <FlaskConical size={13} aria-hidden="true" />
            )}
            {t('settings.mcp.editor.test')}
          </button>
          {(ed.saveError || ed.blocked) && (
            <p
              role="alert"
              className="min-w-0 flex-1 truncate text-caption text-danger"
              title={ed.saveError ?? undefined}
            >
              {ed.saveError ?? t('settings.mcp.editor.fixErrors')}
            </p>
          )}
          <div className="ml-auto flex items-center gap-2">
            <button type="button" className={BTN.subtle} onClick={onClose} disabled={ed.saving}>
              {t('button.cancel')}
            </button>
            <button type="button" className={BTN.primary} onClick={() => void ed.save()} disabled={busy}>
              {ed.saving ? (
                <Loader2 size={13} className="animate-spin" aria-hidden="true" />
              ) : (
                <Save size={13} aria-hidden="true" />
              )}
              {t('settings.mcp.editor.save')}
            </button>
          </div>
        </footer>
      </motion.div>
    </motion.div>,
    document.body,
  )
}
