import { getLocale, t } from '@mika/sdk/i18n'
import { createUiLogger } from '../logger'

const log = createUiLogger('error-message')

/** O Rust escreve `Err(String)` em pt-BR fixo: para quem usa esse idioma o texto cru é nativo. */
const BACKEND_LOCALE = 'pt-BR'

const NETWORK_ERROR_PATTERN = /ECONNREFUSED|ENOTFOUND|EAI_AGAIN|ETIMEDOUT|fetch failed|NetworkError/i

export function isNetworkError(message: string): boolean {
  return NETWORK_ERROR_PATTERN.test(message)
}

const REPORT_TOO_LONG = /^mika:chat-report: 'content' passa de (\d+) caracteres$/

/** Erro do backend com prefixo/forma estável → texto no idioma da UI; `undefined` se não for conhecido. */
function localizeBackendError(raw: string): string | undefined {
  const tooLong = REPORT_TOO_LONG.exec(raw)
  if (tooLong) return t('shell.history.reportTooLong', { max: tooLong[1] })
  return undefined
}

/**
 * Texto de uma rejeição que veio do backend (string crua do `invoke` ou `{ message }` da ponte).
 * Erro de rede passa cru — o chamador o classifica por `isNetworkError` e escolhe o texto. Fora do
 * idioma do backend, o que não tem tradução conhecida NÃO vai para a tela: o detalhe fica só no log.
 */
function backendText(raw: string): string | undefined {
  if (isNetworkError(raw)) return raw
  const localized = localizeBackendError(raw)
  if (localized) return localized
  if (getLocale() === BACKEND_LOCALE) return raw
  log.warn('backend error without localized copy', { detail: raw })
  return undefined
}

function backendMessage(err: unknown): string | undefined {
  const raw = typeof err === 'string' ? err : (err as { message?: unknown } | null)?.message
  return typeof raw === 'string' && raw.trim() ? raw : undefined
}

export function toDisplayError(err: unknown, fallback: string): string
export function toDisplayError(err: unknown, fallback?: undefined): string | undefined
export function toDisplayError(err: unknown, fallback?: string): string | undefined {
  // Error de JavaScript nasceu no renderer: a mensagem é a do próprio código.
  if (err instanceof Error) return err.message
  const raw = backendMessage(err)
  return (raw === undefined ? undefined : backendText(raw)) ?? fallback
}
