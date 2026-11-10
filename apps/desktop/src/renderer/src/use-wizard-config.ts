import type { ConfigChangeEvent } from '@mika/sdk/contracts'
import { useCallback, useEffect, useState } from 'react'
import { getByPath, setByPath } from './components/settings/settings-utils'
import { runtimeApi } from './wizard-app-saves'

// Alimenta o Avatar (`ModelStation`), o Embedding (`EmbeddingProviderPanel`) e a Identidade
// (`AuroraField`) — COMPONENTES DA TELA DE CONFIGURAÇÕES reaproveitados como estão, que leem o valor
// atual pelo par `config`/`configPath`. Sem a config completa (e sem reagir a `config-changed`), o
// clique num skin não acendia o selo "ativo" e o campo de nome voltava a mostrar o rascunho antigo.
export function useWizardConfig() {
  const [config, setConfig] = useState<Record<string, unknown>>({})

  // Mesmo padrão de `useSettingsController`: gravar direto e nunca reler deixaria o selo "ativo" do
  // skin preso no anterior.
  useEffect(() => {
    return runtimeApi().onConfigChanged((event: ConfigChangeEvent) => {
      setConfig((prev) => {
        if (Object.is(getByPath(prev, event.path), event.value)) return prev
        return setByPath(prev, event.path, event.value)
      })
    })
  }, [])

  const saveConfig = useCallback(async (path: string, value: unknown) => {
    const result = await runtimeApi().saveSetting(path, value)
    if (!result.success) throw new Error(result.error || path)
  }, [])

  return { config, setConfig, saveConfig }
}
