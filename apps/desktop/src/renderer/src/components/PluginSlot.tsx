import { type PluginContextValue, PluginProvider } from '@mika/sdk-ui'
import type React from 'react'
import { useState } from 'react'
import { useRegistryStore } from '../store/useRegistryStore'
import { PluginErrorBoundary } from './PluginErrorBoundary'
import { ShadowDOM } from './ShadowDOM'

function formatPluginName(pluginId: string): string {
  const s = pluginId.replace('mika.', '')
  return s.replace(/[._-]/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}

interface PluginSlotProps {
  name: string
  pluginId?: string
  pluginName?: string
  context?: PluginContextValue
  fallback?: React.ReactNode
  children?: React.ReactNode
}

const EMPTY_ARRAY: Array<React.ComponentType<unknown>> = []

export function PluginSlot({ name, pluginId, pluginName, context, fallback, children, ...props }: PluginSlotProps) {
  const [key, setKey] = useState(0)
  const components = useRegistryStore((state) => state.slots[name] || EMPTY_ARRAY)
  const displayName = pluginName ?? (pluginId ? formatPluginName(pluginId) : formatPluginName(name))

  if (components.length === 0) {
    if (children && context) {
      return (
        <PluginErrorBoundary
          key={key}
          pluginId={pluginId ?? name}
          pluginName={displayName}
          onRetry={() => setKey((k) => k + 1)}
        >
          <PluginProvider value={context}>{children}</PluginProvider>
        </PluginErrorBoundary>
      )
    }
    return <>{fallback}</>
  }

  return (
    <>
      {components.map((Component) => {
        const compKey = Component.displayName || Component.name || `${name}-component`
        const compPluginName = compKey

        return (
          <PluginErrorBoundary
            key={`${name}::${compKey}`}
            pluginId={pluginId ?? name}
            pluginName={compPluginName}
            onRetry={() => setKey((k) => k + 1)}
          >
            <ShadowDOM>
              <Component {...props} />
            </ShadowDOM>
          </PluginErrorBoundary>
        )
      })}
    </>
  )
}
