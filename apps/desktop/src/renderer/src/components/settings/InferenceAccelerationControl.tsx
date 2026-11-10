import type { InferenceAcceleration } from '@mika/sdk/contracts'
import type { TranslationDict } from '@mika/sdk/i18n'
import { useI18n } from '../../hooks/useI18n'
import { GlassSelect, StatusPill } from './ds'

type Key = keyof TranslationDict
type Model = InferenceAcceleration['models'][number]

/** `app.inferenceAcceleration` — lido pela política de aceleração ONNX (Rust `modules/inference/accel.rs`). */
const ACCEL_PATH = 'app.inferenceAcceleration'
const OPTIONS: ReadonlyArray<{ value: InferenceAcceleration['preference']; label: Key; hint: Key }> = [
  { value: 'auto', label: 'settings.performance.accel.auto', hint: 'settings.performance.accel.autoHint' },
  { value: 'gpu', label: 'settings.performance.accel.gpu', hint: 'settings.performance.accel.gpuHint' },
  { value: 'cpu', label: 'settings.performance.accel.cpu', hint: 'settings.performance.accel.cpuHint' },
]

/** GPU efetiva = verde; GPU pedida que caiu para CPU = aviso; CPU por política = neutro. */
function pillVariant(model: Model): 'ok' | 'warn' | 'neutral' {
  if (model.reason === 'gpuFailed') return 'warn'
  return model.provider === 'CPU' ? 'neutral' : 'ok'
}

function ModelPill({ model }: { model: Model }) {
  const { t } = useI18n()
  const reason = t(`settings.performance.accel.reason.${model.reason}` as Key)
  return (
    <StatusPill
      variant={pillVariant(model)}
      label={`${t(`settings.performance.accel.model.${model.id}` as Key)} · ${model.provider}`}
      title={model.error ? `${reason}\n${model.error}` : reason}
      dot
    />
  )
}

type Props = {
  config: Record<string, unknown>
  onSave: (path: string, value: string) => Promise<void>
  live: InferenceAcceleration | undefined
}

/** Automática / GPU / CPU + o provedor que cada modelo carregado de fato usa (vem da criação da sessão). */
export function InferenceAccelerationControl({ config, onSave, live }: Props) {
  const { t } = useI18n()
  return (
    <div className="space-y-3">
      <GlassSelect
        label={t('settings.performance.accel.label')}
        configPath={ACCEL_PATH}
        config={config}
        onSave={onSave}
        options={OPTIONS.map(({ value, label, hint }) => ({ value, label: t(label), description: t(hint) }))}
      />
      <div role="status" aria-live="polite" className="flex flex-wrap items-center gap-2">
        <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
          {t('settings.performance.accel.liveLabel')}
        </span>
        {!live ? (
          <StatusPill variant="loading" />
        ) : (
          <>
            <StatusPill
              variant={live.gpuProvider ? 'accent' : 'inactive'}
              label={`${t('settings.performance.accel.gpuLabel')} ${live.gpuProvider ?? t('settings.performance.accel.gpuNone')}`}
            />
            {live.models.length === 0 ? (
              <StatusPill variant="inactive" label={t('settings.performance.accel.none')} />
            ) : (
              live.models.map((model) => <ModelPill key={model.id} model={model} />)
            )}
          </>
        )}
      </div>
    </div>
  )
}
