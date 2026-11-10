import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { CheckIcon, CloseIcon, CloudIcon } from '../Icons'
import { type AssetTask, buildInitialTasks, executeAssetTask } from './wizard-assets'
import type { WizardCopy } from './wizard-copy'
import type { SttSelection } from './wizard-stt'
import type { TtsSelection } from './wizard-tts'

function TaskStatusBadge({ copy, status }: { copy: WizardCopy['assets']; status: AssetTask['status'] }) {
  if (status === 'running') {
    return (
      <span className="flex items-center gap-1.5 text-caption font-medium text-accent">
        <span className="inline-block size-3 animate-spin rounded-full border-2 border-accent border-t-transparent" />
        {copy.statusRunning}
      </span>
    )
  }
  if (status === 'done') {
    return (
      <span className="flex items-center gap-1 text-caption font-medium text-ok">
        <CheckIcon className="size-3.5" />
        {copy.statusDone}
      </span>
    )
  }
  if (status === 'skipped') {
    return (
      <span className="flex items-center gap-1 text-caption font-medium text-text-muted">
        <CloudIcon className="size-3.5" />
        {copy.statusSkipped}
      </span>
    )
  }
  if (status === 'error') {
    return (
      <span className="flex items-center gap-1 text-caption font-medium text-danger">
        <CloseIcon className="size-3.5" />
        {copy.statusError}
      </span>
    )
  }
  return <span className="text-caption text-text-muted">{copy.statusPending}</span>
}

export function AssetPreparationProgress({
  copy,
  llmConfigured = true,
  llmProvider,
  llmModel,
  tts,
  listening,
  stt,
  onReadyStateChange,
}: {
  copy: WizardCopy['assets']
  llmConfigured?: boolean
  llmProvider: string
  llmModel: string
  tts: TtsSelection
  /** Toggle de microfone escolhido no passo de privacidade: sem ele nada de áudio interno baixa. */
  listening: boolean
  stt: SttSelection
  onReadyStateChange: (ready: boolean) => void
}) {
  const isOllama = llmProvider === 'ollama'
  const initialTasks = useMemo(
    () => buildInitialTasks(copy, isOllama, llmConfigured, llmModel, tts, listening, stt),
    [copy, isOllama, llmConfigured, llmModel, tts, listening, stt],
  )
  const [tasks, setTasks] = useState<AssetTask[]>(initialTasks)
  const [isRunning, setIsRunning] = useState(false)
  const ranRef = useRef(false)

  const completedCount = tasks.filter((t) => t.status === 'done' || t.status === 'skipped').length
  const progressPercent = tasks.length > 0 ? Math.round((completedCount / tasks.length) * 100) : 100
  const hasError = tasks.some((t) => t.status === 'error')
  const allTasksSettled = tasks.every((t) => t.status !== 'pending' && t.status !== 'running')
  const isAllFinished = completedCount === tasks.length

  const startPreparation = useCallback(
    async (taskSnapshot: AssetTask[]) => {
      setIsRunning(true)
      onReadyStateChange(false)

      for (const task of taskSnapshot) {
        if (task.status === 'done' || task.status === 'skipped') continue

        setTasks((prev) => prev.map((t) => (t.id === task.id ? { ...t, status: 'running' } : t)))
        const result = await executeAssetTask(copy, task, tts, llmProvider, llmModel)
        setTasks((prev) => prev.map((t) => (t.id === task.id ? { ...t, ...result } : t)))
      }

      setIsRunning(false)
      onReadyStateChange(true)
    },
    [copy, tts, llmProvider, llmModel, onReadyStateChange],
  )

  useEffect(() => {
    if (!ranRef.current) {
      ranRef.current = true
      void startPreparation(initialTasks)
    }
  }, [startPreparation, initialTasks])

  useEffect(() => {
    if (!isRunning && allTasksSettled) {
      onReadyStateChange(true)
    }
  }, [isRunning, allTasksSettled, onReadyStateChange])

  return (
    <div className="mt-2 rounded-xl border border-border-subtle bg-surface-raise p-2 sm:p-2.5">
      <div className="flex items-center justify-between gap-2">
        <div>
          <div className="text-micro font-semibold uppercase tracking-[0.2em] text-text-muted">
            {isAllFinished ? copy.readyEyebrow : copy.preparingEyebrow}
          </div>
          <div className="text-xs font-bold text-text-main">
            {isAllFinished ? copy.readyTitle : copy.preparingTitle}
          </div>
        </div>
        <div className="text-xs font-extrabold text-primary">{progressPercent}%</div>
      </div>

      <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-bg-deep">
        <div
          className="h-full w-full origin-left bg-primary transition-transform duration-300 ease-out"
          style={{ transform: `scaleX(${progressPercent / 100})` }}
        />
      </div>

      <ul className="mt-1.5 max-h-28 overflow-y-auto space-y-1 pr-1">
        {tasks.map((task) => (
          <li
            key={task.id}
            className="flex items-center justify-between gap-2 rounded-lg border border-border-subtle/50 bg-bg-deep/60 px-2 py-1 text-xs"
          >
            <div className="min-w-0 flex-1">
              <div className="text-caption font-semibold text-text-main">{task.label}</div>
              <div className="truncate text-caption text-text-muted">{task.detail}</div>
              {task.errorMessage && <div className="text-caption text-danger">{task.errorMessage}</div>}
            </div>

            <div className="shrink-0">
              <TaskStatusBadge copy={copy} status={task.status} />
            </div>
          </li>
        ))}
      </ul>

      {hasError && !isRunning && (
        <div className="mt-1.5 flex items-center justify-between gap-2 rounded-lg border border-danger-rim bg-danger-soft p-1.5">
          <span className="text-caption leading-3.5 text-danger">{copy.errorHint}</span>
          <button
            type="button"
            onClick={() => void startPreparation(tasks)}
            className="shrink-0 rounded border border-danger-rim bg-surface-raise px-2 py-0.5 text-caption font-semibold text-text-main hover:bg-surface-raise-strong"
          >
            {copy.retry}
          </button>
        </div>
      )}
    </div>
  )
}
