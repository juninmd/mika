/**
 * Bench do hot path de lip-sync do renderer (roda na UI thread ~60x/s enquanto a Mika fala).
 *
 * Uso: `pnpm tsx scripts/dev/bench/lip-sync-hot-path.bench.mts`
 *
 * Mede ns/op (mediana de 3 rodadas, cada uma com aquecimento) de:
 *   - rmsLoudness      (FFT_SIZE amostras por quadro de áudio)
 *   - estimateFormants (espectro de FFT_SIZE/2 bins)
 *   - vowelWeights     (matriz de vogal a partir dos formants)
 *   - ttsLevelWrites   (escritas de CSS var do medidor de TTS em 10 s silenciosos)
 *
 * rodado em Node; o alvo real é WebView2 — os números são comparativos (antes/depois de um PR),
 * não absolutos de dispositivo.
 */
import { performance } from 'node:perf_hooks'
import { ttsLevelWrite } from '../../../apps/desktop/src/renderer/src/components/hud/tts-level-write'
import { estimateFormants, rmsLoudness, vowelWeights } from '../../../apps/desktop/src/renderer/src/lib/lip-sync-vowels'

const FFT_SIZE = 2048
const SAMPLE_RATE = 48_000

const timeDomain = new Float32Array(FFT_SIZE)
for (let i = 0; i < FFT_SIZE; i++) timeDomain[i] = Math.sin(i / 10) * 0.3

const spectrum = new Uint8Array(FFT_SIZE / 2)
spectrum.fill(12)
spectrum[Math.round(750 / (SAMPLE_RATE / FFT_SIZE))] = 220
spectrum[Math.round(1300 / (SAMPLE_RATE / FFT_SIZE))] = 180

const FORMANTS = estimateFormants(spectrum, SAMPLE_RATE, FFT_SIZE)
const LOUDNESS = 0.4

/** Mínimo de 10 rodadas de ~250ms: cada rodada roda `fn` e devolve ns/op. O mínimo amacia o ruído
 *  da máquina (GC, agendador) a cada rodada inteira. */
function bench(fn: () => void, targetMs = 250): number {
  let best = Number.POSITIVE_INFINITY
  for (let round = 0; round < 10; round++) {
    // Aquecimento (JIT: reoptimização acontece nas primeiras milhares de chamadas).
    for (let i = 0; i < 5_000; i++) fn()
    let ops = 0
    const start = performance.now()
    let now = start
    do {
      fn()
      ops++
      now = performance.now()
    } while (now - start < targetMs)
    const nsPer = ((now - start) * 1e6) / ops
    if (nsPer < best) best = nsPer
  }
  return best
}

/** Escritor de CSS var do medidor de TTS — VERSÃO ANTIGA (inline hoje em useLevelVar): escreve todo frame. */
function oldLevelWrite(smooth: number, write: () => void): void {
  write(Math.min(1, Math.max(0, smooth)).toFixed(3))
}

/** O medidor de TTS, medido no estado real mais incomum: status "speaking"
 *  travado depois do fim do áudio (cauda suave → silêncio). O código antigo escreve em cada
 *  frame; o novo pula quando o valor quantizado não mudou. Devolve escritas/s. */
function benchLevelWrites(mode: 'old' | 'new'): number {
  let writes = 0
  let last = -1
  let smooth = 0.35
  const write = () => {
    writes++
  }
  const start = performance.now()
  for (let frame = 0; frame < 600; frame++) {
    // volumeRef a 0 (áudio acabou), suavização igual ao uso real em useLevelVar.
    smooth = smooth * 0.55 + 0 * 0.45
    if (mode === 'old') {
      oldLevelWrite(smooth * 1.6, write)
      continue
    }
    const q = ttsLevelWrite(smooth * 1.6, last)
    if (q !== last) {
      last = q
      write()
    }
  }
  const ms = performance.now() - start
  // O custo que interessa: escritas de CSS var na janela de 10 s (600 frames @ 60 fps).
  void ms
  return writes
}

function fmt(ns: number): string {
  return ns >= 1000 ? `${(ns / 1000).toFixed(2)} µs/op` : `${ns.toFixed(0)} ns/op}`
}

const rms = bench(() => rmsLoudness(timeDomain))
const formants = bench(() => estimateFormants(spectrum, SAMPLE_RATE, FFT_SIZE))
const vowels = bench(() => vowelWeights(FORMANTS, LOUDNESS))
const writesOld = benchLevelWrites('old')
const writesNew = benchLevelWrites('new')

// `--json` imprime linha única parseável (min-agregação entre processos fica no runner, ex. pwsh).
if (process.argv.includes('--json')) {
  console.log(JSON.stringify({ rms, formants, vowels, writesOld, writesNew }))
} else {
  console.log(`rmsLoudness(2048 amostras):   ${fmt(rms)}`)
  console.log(`estimateFormants(1023 bins):  ${fmt(formants)}`)
  console.log(`vowelWeights:                 ${fmt(vowels)}`)
  console.log(
    `ttsLevelWrites (@60fps, 10s = 600 frames): antigo ${writesOld.toFixed(0)}/600 · novo ${writesNew.toFixed(0)}/600`,
  )
}
