import type { ToolCallOrigin } from '@mika/sdk/svc/events'

// Only turn tools light up the thinking HUD; missing `origin` defaults to 'turn' so old/plugin events never go silent.
export function isForegroundToolEvent(origin: ToolCallOrigin | undefined): boolean {
  return origin !== 'background'
}
