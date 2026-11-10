import { useI18n } from '../../hooks/useI18n'
import { Tooltip, type useTooltipTrigger } from '../ui/Tooltip'

type DockDragHandleProps = {
  dragTooltipVisible: boolean
  dragTriggerProps: ReturnType<typeof useTooltipTrigger>['triggerProps']
}

export function DockDragHandle({ dragTooltipVisible, dragTriggerProps }: DockDragHandleProps) {
  const { t } = useI18n()
  return (
    <div
      className="group relative flex h-10 w-6 cursor-grab items-center justify-center rounded-xl bg-surface-raise hover:bg-surface-raise-strong active:cursor-grabbing transition border border-transparent hover:border-border-strong shrink-0 self-center"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      aria-hidden="true"
      {...dragTriggerProps}
    >
      <Tooltip label={t('shell.dock.dragHint')} visible={dragTooltipVisible} />
      <div className="flex gap-[3px]">
        <div className="flex flex-col gap-[3px]">
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
        </div>
        <div className="flex flex-col gap-[3px]">
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
          <span className="h-1 w-1 rounded-full bg-text-muted/60 group-hover:bg-info transition-colors" />
        </div>
      </div>
    </div>
  )
}
