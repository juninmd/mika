/**
 * `legal` pré-aceito para o `config.json` dos fixtures. Sem ele o app abre com o diálogo bloqueante
 * de EULA/privacidade (`LegalGate`) por cima de tudo e nenhum spec de UI encontra o que procura.
 *
 * A versão é a MESMA que `modules/legal.rs::acceptance_version` calcula: 16 hex de
 * SHA-256(EULA ␀ PRIVACY), com CRLF normalizado para LF. Se os dois lados divergirem o diálogo
 * aparece nos specs (falha visível, não silenciosa) — o valor dourado do teste Rust
 * `acceptance_version_follows_content_not_line_endings` é o ponto de conferência.
 */
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const REPO_ROOT = join(__dirname, '..', '..', '..', '..')
const read = (file: string) => readFileSync(join(REPO_ROOT, file), 'utf8').replace(/\r\n/g, '\n')

export function legalAccepted(): { acceptedVersion: string; acceptedAt: string; ageConfirmed: boolean } {
  const acceptedVersion = createHash('sha256')
    .update(read('EULA.md'))
    .update(Buffer.from([0]))
    .update(read('PRIVACY.md'))
    .digest('hex')
    .slice(0, 16)
  // `ageConfirmed` is part of the same acceptance ("18 anos ou mais"): without it the gate reopens.
  return { acceptedVersion, acceptedAt: '2026-01-01T00:00:00.000Z', ageConfirmed: true }
}
