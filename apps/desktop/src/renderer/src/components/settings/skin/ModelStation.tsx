import { toDisplayError } from '@mika/sdk-ui'
import { Box, FolderCog } from 'lucide-react'
import { type KeyboardEvent, useRef, useState } from 'react'
import { type Translate, useI18n } from '../../../hooks/useI18n'
import { useUIStore } from '../../../store/useUIStore'
import { LIVE2D_MODELS, type Live2DModelOption, resolveLive2DModelPath, VRM_MODELS } from '../../live2d/model-path'
import { AuroraField, GlassSection, SectionLabel } from '../ds'
import { configStr } from '../settings-utils'
import { RosterCard } from './RosterCard'
import { rosterKeyTarget } from './roster-nav'
import { SkinDetails } from './SkinDetails'
import { SkinStage } from './SkinStage'
import { skinFacts } from './skin-catalog'

const ROSTER: readonly Live2DModelOption[] = [...VRM_MODELS, ...LIVE2D_MODELS]

// The only label that is not a proper name; keeps the "(VRM)" suffix its siblings carry.
function resolveModelLabel(model: Live2DModelOption, t: Translate): string {
  return model.id === 'vrm-original' ? `${t('settings.skin.modelStation.originalLook')} (VRM)` : model.label
}

/** Roster cards shown on the first grid row: the column count arrow Up/Down jump by, at any width. */
function gridColumns(grid: HTMLElement | null): number {
  const cards = Array.from(grid?.children ?? []) as HTMLElement[]
  const top = cards[0]?.offsetTop
  return Math.max(1, cards.filter((card) => card.offsetTop === top).length)
}

type ModelStationProps = {
  config: Record<string, unknown>
  save: (path: string, value: unknown) => Promise<void>
}

function useApply(save: ModelStationProps['save'], t: Translate) {
  const [applyingId, setApplyingId] = useState<string | null>(null)
  const apply = async (model: Live2DModelOption) => {
    setApplyingId(model.id)
    try {
      await save('character.modelPath', model.path)
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.skin.modelStation.swapError')))
    } finally {
      setApplyingId(null)
    }
  }
  return { applyingId, apply }
}

// Character select: live stage + details of the previewed skin on top, roster underneath. Browsing
// (click, arrows) only previews; "Aplicar", Enter or double-click saves `character.modelPath` and
// reloads the avatar — the same key and value the old one-click grid wrote.
export function ModelStation({ config, save }: ModelStationProps) {
  const { t } = useI18n()
  const { applyingId, apply } = useApply(save, t)
  const appliedPath = resolveLive2DModelPath({
    modelName: configStr(config, 'character.modelName'),
    modelPath: configStr(config, 'character.modelPath'),
  })
  const [previewId, setPreviewId] = useState(() => (ROSTER.find((m) => m.path === appliedPath) ?? ROSTER[0])?.id)
  const gridRef = useRef<HTMLDivElement>(null)
  const cardRefs = useRef<Array<HTMLButtonElement | null>>([])
  const previewed = ROSTER.find((m) => m.id === previewId) ?? (ROSTER[0] as Live2DModelOption)
  const facts = skinFacts(previewed)
  const label = resolveModelLabel(previewed, t)

  const handleKeyDown = (e: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      void apply(ROSTER[index] as Live2DModelOption)
      return
    }
    const next = rosterKeyTarget(e.key, index, ROSTER.length, gridColumns(gridRef.current))
    if (next === null) return
    e.preventDefault()
    setPreviewId(ROSTER[next]?.id)
    cardRefs.current[next]?.focus()
  }

  return (
    <>
      <GlassSection
        title={t('settings.skin.modelStation.title')}
        icon={Box}
        description={t('settings.skin.modelStation.description')}
      >
        <div className="space-y-5">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1.55fr)_minmax(260px,1fr)]">
            <SkinStage model={previewed} label={label} facts={facts} />
            <SkinDetails
              model={previewed}
              label={label}
              facts={facts}
              applied={previewed.path === appliedPath}
              applying={applyingId === previewed.id}
              onApply={() => void apply(previewed)}
            />
          </div>
          <div className="space-y-3">
            <SectionLabel count={ROSTER.length}>{t('settings.skin.roster.title')}</SectionLabel>
            <div
              ref={gridRef}
              role="radiogroup"
              aria-label={t('settings.skin.roster.title')}
              className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-6"
            >
              {ROSTER.map((model, index) => (
                <RosterCard
                  key={model.id}
                  model={model}
                  label={resolveModelLabel(model, t)}
                  kind={skinFacts(model).kind}
                  previewed={model.id === previewed.id}
                  applied={model.path === appliedPath}
                  applying={applyingId === model.id}
                  cardRef={(node) => {
                    cardRefs.current[index] = node
                  }}
                  onPreview={() => setPreviewId(model.id)}
                  onApply={() => void apply(model)}
                  onKeyDown={(e) => handleKeyDown(e, index)}
                />
              ))}
            </div>
            <p className="text-caption text-text-muted">{t('settings.skin.roster.hint')}</p>
          </div>
        </div>
      </GlassSection>

      <GlassSection
        title={t('settings.skin.modelStation.customPathLabel')}
        icon={FolderCog}
        description={t('settings.skin.modelStation.customPathDescription')}
        collapsible
        defaultCollapsed
      >
        <AuroraField
          label={t('settings.skin.modelStation.customPathLabel')}
          configPath="character.modelPath"
          config={config}
          onSave={save}
          placeholder={t('settings.skin.modelStation.customPathPlaceholder')}
        />
      </GlassSection>
    </>
  )
}
