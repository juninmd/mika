import { describe, expect, it } from 'vitest'
import { releaseNotes } from '../release-notes.mts'

describe('releaseNotes — o changelog que o updater mostra sai dos commits convencionais', () => {
  it('agrupa por tipo, na ordem quebra → novidade → correção → desempenho', () => {
    const notes = releaseNotes([
      'fix(voice): corta eco no barge-in',
      'feat(settings): exporta backup em TOML',
      'perf(tts): reusa conexão HTTP entre frases',
      'feat!: remove canal legado\n\nBREAKING CHANGE: x',
    ])
    expect(notes).toBe(
      [
        '## ⚠️ Mudanças incompatíveis',
        '- remove canal legado',
        '',
        '## ✨ Novidades',
        '- **settings:** exporta backup em TOML',
        '',
        '## 🐛 Correções',
        '- **voice:** corta eco no barge-in',
        '',
        '## ⚡ Desempenho',
        '- **tts:** reusa conexão HTTP entre frases',
      ].join('\n'),
    )
  })

  it('chore, ci, docs, test e merge não entram: o usuário não ganha nada lendo isso', () => {
    expect(releaseNotes(['chore: deps', 'ci: cache', "Merge branch 'x'", 'docs: readme', 'test: y'])).toBe('')
  })

  it('só o título do commit (primeira linha) vira item; o sufixo "(#123)" do squash sai', () => {
    expect(releaseNotes(['fix: corrige o tray (#470)\n\ncorpo longo'])).toBe('## 🐛 Correções\n- corrige o tray')
  })
})
