import { ConfigKey } from '@mika/sdk/contracts'
import { getElectronApi } from './shell-api'

/**
 * Aceite do EULA + política de privacidade. A versão vem do Rust (`modules/legal.rs`, hash do
 * conteúdo empacotado); aqui só se compara com o que está gravado em `legal.acceptedVersion`.
 * Versão desconhecida (`''`, ainda carregando) NUNCA conta como aceita, senão o default vazio do
 * schema bateria com ela e o diálogo sumiria sem ninguém aceitar nada. O aceite só está completo com
 * `legal.ageConfirmed` ("Tenho 18 anos ou mais"): o app é para adultos (EULA §9, política §11).
 */
export function isLegalAccepted(config: Record<string, unknown>, currentVersion: string): boolean {
  const legal = config.legal as Record<string, unknown> | undefined
  return currentVersion !== '' && legal?.acceptedVersion === currentVersion && legal?.ageConfirmed === true
}

export function saveLegalAcceptance(version: string, now: Date = new Date()) {
  const { saveSetting } = getElectronApi().runtime
  return Promise.all([
    saveSetting(ConfigKey.LegalAcceptedVersion, version),
    saveSetting(ConfigKey.LegalAcceptedAt, now.toISOString()),
    saveSetting(ConfigKey.LegalAgeConfirmed, true),
  ])
}

/** Recusar os termos = sair do app (a bandeja e o avatar continuam vivos se só a janela fechasse). */
export async function quitApp(): Promise<void> {
  await getElectronApi().shell.system?.quit?.()
}
