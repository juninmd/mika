/**
 * Inventário de licenças do que o instalador carrega: dependências Rust (`cargo metadata`), npm
 * (`pnpm licenses list --prod --json`) e o inventário MANUAL de assets e modelos, que nenhum
 * gerenciador de pacotes enxerga (modelos de voz, VRM, motions, runtime Live2D).
 *
 * Fonte única de `scripts/generators/third-party-notices.mts` (escreve `THIRD-PARTY-NOTICES.md`) e
 * de `scripts/checks/license-policy.mts` (reprova licença proibida e arquivo desatualizado).
 */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, realpathSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'

export const REPO_ROOT = resolve(import.meta.dirname, '..', '..')
export const NOTICES_FILE = 'THIRD-PARTY-NOTICES.md'
export const TEXTS_DIR = 'scripts/generators/license-texts'
/** Sobe quando o FORMATO do arquivo gerado muda — invalida o hash mesmo sem mudança de lockfile. */
export const GENERATOR_VERSION = 1

const CARGO_MANIFEST = 'apps/desktop/src-tauri/Cargo.toml'
const LICENSE_FILE = /^(?:LICEN[CS]E|COPYING|UNLICENSE|NOTICE)(?:[-_.].*)?$/i
const MAX_LICENSE_BYTES = 120_000

export type Component = {
  ecosystem: 'rust' | 'npm'
  name: string
  version: string
  /** Expressão SPDX como declarada pelo pacote. */
  license: string
  repository: string
  /** Textos de licença/aviso encontrados na raiz do pacote. */
  texts: string[]
}

export type Delivery = 'bundled' | 'on-demand' | 'user-supplied' | 'steam-build' | 'not-shipped'

export type ManualAsset = {
  name: string
  role: string
  license: string
  holder: string
  source: string
  delivery: Delivery
  note: string
  /** Arquivo em `TEXTS_DIR` com o texto integral, quando a licença é redistribuível. */
  textFile?: string
}

/**
 * Inventário manual. Cada afirmação aqui foi conferida na fonte (metadados embutidos no arquivo, API
 * do Hugging Face, LICENSE do repositório) — o que NÃO foi conferido diz "verificar" na nota, e é a
 * lista de pendências jurídicas, nunca uma licença presumida.
 */
export const MANUAL_ASSETS: readonly ManualAsset[] = [
  {
    name: 'Live2D Cubism Core (live2dcubismcore.min.js)',
    role: 'Runtime JavaScript que anima avatares Live2D',
    license: 'Live2D Proprietary Software License Agreement',
    holder: 'Live2D Inc.',
    source: 'https://www.live2d.com/en/sdk/license/',
    delivery: 'bundled',
    note: 'Runtime redistribuível, não material. Verificar se a publicação comercial exige o contrato de publicação do Cubism SDK (Release License) para a faixa de faturamento da empresa.',
  },
  {
    name: 'Modelos de amostra Live2D (haru, Mao)',
    role: 'Avatares Live2D de exemplo, só no repositório de desenvolvimento',
    license: 'Live2D Free Material License Agreement',
    holder: 'Live2D Inc.',
    source: 'https://www.live2d.com/eula/live2d-free-material-license-agreement_en.html',
    delivery: 'not-shipped',
    note: 'Removidos do bundle pelo passo closeBundle de vite.tauri.config.ts. O usuário os instala por conta própria em <dados do app>/live2d/<nome>/.',
  },
  {
    name: 'mika.vrm (Mika Aurora — Arcade, derivada de VRM1_Constraint_Twist_Sample)',
    role: 'Avatar 3D oficial padrão',
    license:
      'VRM Public License 1.0 — metadados embutidos: commercialUsage=corporation, allowRedistribution=true, modificação e redistribuição permitidas, crédito dispensável, uso antissocial/discurso de ódio proibido',
    holder: '(c) 2022 pixiv Inc.',
    source: 'https://vrm.dev/licenses/1.0/',
    delivery: 'bundled',
    note: 'Roupa Arcade e ajustes visuais do projeto Mika; termos retidos da base e lidos do bloco VRMC_vrm.meta. A base original fica preservada em scripts/generators/assets/mika-base.vrm para regenerar Haru.',
  },
  {
    name: 'haru.vrm (Mika Haru — Celestial Greeter)',
    role: 'Variante 3D com roupa e cabelo procedurais',
    license: 'VRM Public License 1.0 (mesmos metadados da base)',
    holder: '(c) 2022 pixiv Inc. (base); roupa e cabelo procedurais: projeto Mika',
    source: 'https://vrm.dev/licenses/1.0/',
    delivery: 'bundled',
    note: 'Derivada da base imutável scripts/generators/assets/mika-base.vrm por scripts/generators/generate-haru-vrm.mjs; não usa o mika.vrm oficial, e a permissão de modificação e redistribuição vem da base.',
  },
  {
    name: 'Clipes VRMA: Angry, Blush, Clapping, Goodbye, LookAround, Relax, Sad, Sleepy, Surprised, Thinking',
    role: 'Animações de gestos do avatar 3D',
    license: 'MIT',
    holder: 'tk256ailab (vrm-viewer)',
    source: 'https://github.com/tk256ailab/vrm-viewer',
    delivery: 'bundled',
    note: 'Conforme apps/desktop/src/renderer/public/vrm/animations/CREDITS.md. Verificar o texto MIT do repositório de origem e anexá-lo.',
  },
  {
    name: 'Clipe VRMA: idle_loop',
    role: 'Animação de repouso do avatar 3D',
    license: 'MIT',
    holder: 'pixiv Inc. (ChatVRM), obtido via semperai/amica',
    source: 'https://github.com/pixiv/ChatVRM',
    delivery: 'bundled',
    note: 'Conforme CREDITS.md. Verificar o texto MIT do repositório de origem e anexá-lo.',
  },
  {
    name: 'Clipes VRMA: modelPose, peaceSign, shoot',
    role: 'Animações de gestos do avatar 3D',
    license: 'Termos do VRoid Project (crédito obrigatório)',
    holder: '(c) pixiv Inc.',
    source: 'https://github.com/semperai/amica',
    delivery: 'bundled',
    note: 'Termos NÃO conferidos na fonte oficial nesta rodada. Verificar uso comercial e a redação exata do crédito antes de publicar; na dúvida, substituir por clipes próprios.',
  },
  {
    name: 'Fontes Inter (variável), IBM Plex Mono e Space Grotesk (@fontsource)',
    role: 'Tipografia da interface, empacotada localmente (sem Google Fonts)',
    license: 'OFL-1.1',
    holder: 'Conforme o arquivo LICENSE de cada pacote (seção de dependências npm)',
    source: 'https://fontsource.org',
    delivery: 'bundled',
    note: 'SIL Open Font License 1.1: uso comercial e redistribuição permitidos; proíbe vender as fontes isoladamente e usar os nomes reservados em derivados modificados.',
  },
  {
    name: 'ONNX Runtime',
    role: 'Executa os modelos locais de voz e embeddings (via crate ort)',
    license: 'MIT',
    holder: 'Microsoft Corporation',
    source: 'https://github.com/microsoft/onnxruntime',
    delivery: 'bundled',
    note: 'Binário baixado pelo build do crate ort. No Windows, o build inclui o provedor DirectML: verificar os termos de redistribuição da DLL DirectML da Microsoft. Os builds CUDA/TensorRT (features opcionais) NÃO fazem parte do instalador padrão.',
    textFile: 'onnxruntime-mit.txt',
  },
  {
    name: 'Silero VAD (silero_vad.onnx)',
    role: 'Detecção de atividade de voz local',
    license: 'MIT',
    holder: 'Silero Team (2020-present)',
    source: 'https://github.com/snakers4/silero-vad',
    delivery: 'on-demand',
    note: 'Baixado no primeiro uso, de commit fixo, com a ação do usuário no assistente de configuração.',
    textFile: 'silero-vad-mit.txt',
  },
  {
    name: 'Whisper tiny/base/small (conversão ONNX onnx-community)',
    role: 'Reconhecimento de fala local',
    license: 'Apache-2.0 (conforme declarado em openai/whisper-base); código do Whisper: MIT',
    holder: 'OpenAI',
    source: 'https://huggingface.co/onnx-community/whisper-base',
    delivery: 'on-demand',
    note: 'Baixado do Hugging Face em revisão fixa. O cartão da conversão onnx-community não declara licença própria: vale a do modelo original openai/whisper-base.',
    textFile: 'apache-2.0.txt',
  },
  {
    name: 'Nemotron 3.5 ASR Streaming 0.6B (int4 ONNX)',
    role: 'Reconhecimento de fala local padrão',
    license:
      'CONFLITO DE DECLARAÇÕES — repositório NVIDIA de origem: OpenMDW-1.1; repositório da conversão ONNX: "license: mit" com license_name nvidia-open-model-license',
    holder: 'NVIDIA Corporation (modelo); onnx-community (conversão)',
    source: 'https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b',
    delivery: 'on-demand',
    note: 'Pendência jurídica: confirmar qual termo governa a conversão distribuída. Até lá, o texto da OpenMDW-1.1 (que exige reter a licença e os avisos de origem) é anexado e a NVIDIA Open Model License deve ser lida em https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/.',
    textFile: 'openmdw-1.1.txt',
  },
  {
    name: 'Supertonic-TTS-2 (ONNX)',
    role: 'Voz local padrão (TTS)',
    license: 'BigScience OpenRAIL-M (modelo); MIT (código de exemplo)',
    holder: 'Copyright (c) 2026 Supertone Inc.',
    source: 'https://huggingface.co/Supertone/supertonic-2',
    delivery: 'on-demand',
    note: 'A OpenRAIL-M impõe restrições de uso (Attachment A) que o distribuidor DEVE repassar aos usuários (cláusula 5): o EULA as incorpora. Revisão fixa no Hugging Face.',
    textFile: 'openrail-m-bigscience.txt',
  },
  {
    name: 'Kokoro-82M v1.0 (ONNX)',
    role: 'Voz local alternativa (opção hoje fora da interface)',
    license: 'Apache-2.0',
    holder: 'hexgrad (modelo original); onnx-community (conversão)',
    source: 'https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX',
    delivery: 'on-demand',
    note: 'O código de download existe; o provedor está cortado de dispatch.rs e não aparece nas configurações.',
    textFile: 'apache-2.0.txt',
  },
  {
    name: 'multilingual-e5-small (Xenova, conversão ONNX)',
    role: 'Embeddings locais da memória semântica',
    license: 'MIT (conforme declarado em intfloat/multilingual-e5-small)',
    holder: 'intfloat (modelo original); Xenova (conversão)',
    source: 'https://huggingface.co/Xenova/multilingual-e5-small',
    delivery: 'on-demand',
    note: 'O cartão da conversão não declara licença própria. Verificar o titular do copyright e anexar o texto MIT do repositório original.',
  },
  {
    name: 'Piper (binário externo) e vozes rhasspy/piper-voices',
    role: 'Voz local opcional (TTS)',
    license: 'Repositório de vozes: MIT. Licenças dos DATASETS variam por voz',
    holder: 'rhasspy; autores dos datasets de cada voz',
    source: 'https://huggingface.co/rhasspy/piper-voices',
    delivery: 'user-supplied',
    note: 'O binário do Piper não é distribuído: o usuário o instala. Vozes oferecidas (catálogo com SHA-256 na revisão 375a0fe do repositório, apps/desktop/src-tauri/src/modules/voice/tts/providers/piper/catalog.rs): pt_BR-faber-medium (dataset CC0), pt_BR-edresson-low (dataset CC BY 4.0 — exige atribuição), de_DE-thorsten-medium (dataset Thorsten-Voice, CC0), it_IT-paola-medium (dataset paolapersico1/Voice-Dataset-Italian, CC0-1.0) e ru_RU-denis-medium (dataset NabuCasa/voice-datasets, CC0). PENDÊNCIA JURÍDICA: faber, thorsten, paola e denis são ajustes finos do checkpoint en_US-lessac, treinado no dataset Lessac/Blizzard 2013, cuja licença é só para pesquisa e exclui uso comercial, inclusive o desenvolvimento de produtos de síntese de voz — a linhagem dos pesos precisa de parecer antes de vender. en_US-lessac-high (treinada direto nesse dataset) e en_US-amy-medium (licença do dataset não verificada) foram RETIRADAS da interface e do catálogo. Sem voz Piper oferecida para en (o Supertonic já fala), ja e ko (o Piper não tem voz nesses idiomas) nem zh (zh_CN-huayan: licença do dataset "Unknown"); irina (ru: "Unknown"), ruslan (ru: CC BY-NC-SA) e pavoque (de: CC BY-NC-SA) NÃO podem ser oferecidas num produto vendido. Antes de ampliar a lista, conferir o MODEL_CARD e a origem de cada voz.',
  },
  {
    name: 'Steamworks SDK (steam_api64.dll / libsteam_api.so)',
    role: 'Integração com o cliente Steam (Rich Presence, conquistas)',
    license: 'Valve Steamworks SDK Access Agreement',
    holder: 'Valve Corporation',
    source: 'https://partner.steamgames.com/',
    delivery: 'steam-build',
    note: 'Só no build compilado com a feature steam. Redistribuível apenas nos termos do contrato de parceiro Steamworks; o crate steamworks (binding) é MIT/Apache-2.0 e consta na lista Rust.',
  },
  {
    name: 'Ícone do aplicativo (apps/desktop/build/icon.png, icon.ico)',
    role: 'Identidade visual',
    license: 'Proprietária — todos os direitos reservados',
    holder: '[RAZÃO SOCIAL]',
    source: 'scripts/generators/generate-app-icon.mts',
    delivery: 'bundled',
    note: 'Obra autoral gerada pelo projeto a partir do próprio avatar. Confirmar a cadeia de titularidade do avatar-base.',
  },
]

const sha256 = (value: string | Buffer): string => createHash('sha256').update(value).digest('hex')
const readRepo = (rel: string, root = REPO_ROOT): Buffer => readFileSync(join(root, rel))

/** Hash do texto SEM depender do fim de linha: o checkout no Windows (`core.autocrlf`) troca LF por CRLF
 * e os bytes crus mudam sem o conteúdo mudar — o gate reprovava `Cargo.lock` idêntico. */
const sha256Text = (value: Buffer): string => sha256(value.toString('utf8').replace(/\r\n/g, '\n'))

/** Hash de tudo que muda o conteúdo do arquivo gerado — o gate compara com o cabeçalho. */
export function inputHash(root = REPO_ROOT): string {
  const parts = [
    `generator:${GENERATOR_VERSION}`,
    `cargo.lock:${sha256Text(readRepo('Cargo.lock', root))}`,
    `pnpm-lock:${sha256Text(readRepo('pnpm-lock.yaml', root))}`,
    `manual:${sha256(JSON.stringify(MANUAL_ASSETS))}`,
  ]
  for (const file of readdirSync(join(root, TEXTS_DIR), { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .sort()) {
    parts.push(`${file}:${sha256Text(readRepo(`${TEXTS_DIR}/${file}`, root))}`)
  }
  return sha256(parts.join('\n'))
}

function readLicenseTexts(dir: string): string[] {
  let names: string[]
  try {
    names = readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && LICENSE_FILE.test(entry.name))
      .map((entry) => entry.name)
      .sort()
  } catch {
    return []
  }
  const texts: string[] = []
  for (const name of names) {
    const raw = readFileSync(join(dir, name)).subarray(0, MAX_LICENSE_BYTES).toString('utf8')
    const text = raw.replace(/\r\n/g, '\n').trim()
    if (text !== '') texts.push(text)
  }
  return texts
}

type CargoPackage = {
  id: string
  name: string
  version: string
  license: string | null
  repository: string | null
  manifest_path: string
}

type CargoNode = { id: string; deps: { pkg: string; dep_kinds: { kind: string | null }[] }[] }

/** Pacotes alcançáveis por dependência NORMAL (não dev, não build) a partir do crate do app. */
function reachableRustPackages(): CargoPackage[] {
  const output = execFileSync(
    'cargo',
    ['metadata', '--format-version', '1', '--locked', '--all-features', '--manifest-path', CARGO_MANIFEST],
    { cwd: REPO_ROOT, maxBuffer: 256 * 1024 * 1024, encoding: 'utf8' },
  )
  const metadata = JSON.parse(output) as {
    packages: CargoPackage[]
    workspace_members: string[]
    resolve: { nodes: CargoNode[] }
  }
  const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]))
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]))
  const seen = new Set<string>()
  const stack = [...metadata.workspace_members]
  while (stack.length > 0) {
    const id = stack.pop() as string
    if (seen.has(id)) continue
    seen.add(id)
    for (const dep of nodes.get(id)?.deps ?? []) {
      if (dep.dep_kinds.some((kind) => kind.kind === null)) stack.push(dep.pkg)
    }
  }
  for (const member of metadata.workspace_members) seen.delete(member)
  return [...seen].map((id) => packages.get(id) as CargoPackage)
}

const byNameVersion = (a: Component, b: Component): number =>
  a.name.localeCompare(b.name) || a.version.localeCompare(b.version)

export function collectRust(withTexts: boolean): Component[] {
  return reachableRustPackages()
    .map(
      (pkg): Component => ({
        ecosystem: 'rust',
        name: pkg.name,
        version: pkg.version,
        license: pkg.license ?? '',
        repository: pkg.repository ?? '',
        texts: withTexts ? readLicenseTexts(dirname(pkg.manifest_path)) : [],
      }),
    )
    .sort(byNameVersion)
}

type PnpmEntry = { name: string; versions: string[]; paths: string[]; license: string; homepage?: string }

/** Quando o `package.json` não declara licença, o texto do arquivo ainda diz qual é (khroma: MIT). */
function sniffLicense(texts: string[]): string {
  const text = texts.join('\n')
  if (/Permission is hereby granted, free of charge/i.test(text)) return 'MIT'
  if (/Apache License\s+Version 2\.0/i.test(text)) return 'Apache-2.0'
  if (/Redistribution and use in source and binary forms/i.test(text)) return 'BSD-3-Clause'
  return ''
}

export function collectNpm(withTexts: boolean): Component[] {
  const output = execFileSync('pnpm', ['licenses', 'list', '--prod', '--json'], {
    cwd: REPO_ROOT,
    maxBuffer: 256 * 1024 * 1024,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  })
  const groups = JSON.parse(output) as Record<string, PnpmEntry[]>
  const components: Component[] = []
  for (const entries of Object.values(groups)) {
    for (const entry of entries) {
      entry.versions.forEach((version, index) => {
        const dir = realpathSync(entry.paths[index] ?? (entry.paths[0] as string))
        const texts = readLicenseTexts(dir)
        const declared = /^unknown$/i.test(entry.license) ? sniffLicense(texts) : entry.license
        components.push({
          ecosystem: 'npm',
          name: entry.name,
          version,
          license: declared,
          repository: entry.homepage ?? '',
          texts: withTexts ? texts : [],
        })
      })
    }
  }
  return components.sort(byNameVersion)
}

export { sha256 }
