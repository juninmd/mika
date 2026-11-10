/** Renderiza o VRM e o adaptador reais no Chromium headless, sem iniciar o shell Tauri. */

import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { resolve } from 'node:path'
import { build } from 'esbuild'
import { chromium } from 'playwright'
import type { VrmAvatar } from '../../apps/desktop/src/renderer/src/components/avatar/vrm-adapter'

type StudioWindow = Window & {
  studio: {
    durations: Record<string, number>
    volume: { current: number }
    vowels: { current: { a: number; i: number; u: number; e: number; o: number } }
    avatar: Pick<
      VrmAvatar,
      'camera' | 'update' | 'render' | 'playAction' | 'readArmRotations' | 'setExpression' | 'setSpeaking'
    > & {
      renderer: {
        info: { autoReset: boolean; reset(): void; render: { calls: number; triangles: number } }
      }
    }
  }
}
const video = process.argv.includes('--video')
const artOnly = process.argv.includes('--art-only')
const modelPath = process.argv.find((arg) => arg.startsWith('--model='))?.slice(8) ?? 'vrm/mika/haru.vrm'
const destinationArg = process.argv.find((arg) => arg.startsWith('--destination='))?.slice(14)
const browserChannel = process.argv.find((arg) => arg.startsWith('--browser='))?.slice(10)

const destination = resolve(
  destinationArg ?? (artOnly ? '.homologation/haru-art' : 'apps/desktop/e2e/screenshots/haru-studio'),
)
await mkdir(destination, { recursive: true })
const bundle = await build({
  stdin: {
    contents: `
      import { VrmAvatar } from './apps/desktop/src/renderer/src/components/avatar/vrm-adapter';
      import { BODY_ACTION_SECONDS } from './apps/desktop/src/renderer/src/components/avatar/vrm-body-rig';
      const canvas = document.querySelector('canvas');
      const avatar = new VrmAvatar(canvas, 720, 900);
      avatar.setIdleEnabled(false);
      await avatar.load('/avatar.vrm');
      const volume = { current: 0 };
      const vowels = { current: { a: 0, i: 0, u: 0, e: 0, o: 0 } };
      avatar.setLipSyncSources(volume, vowels);
      window.studio = { avatar, durations: BODY_ACTION_SECONDS, volume, vowels };
      avatar.update(1 / 60); avatar.render();
    `,
    resolveDir: process.cwd(),
  },
  bundle: true,
  format: 'esm',
  write: false,
  logLevel: 'silent',
})
const model = await readFile(resolve('apps/desktop/src/renderer/public', modelPath))
const html =
  '<body style="margin:0;background:#191b2c"><canvas style="width:720px;height:900px"></canvas><script type="module" src="/studio.js"></script></body>'
const server = createServer((req, res) => {
  res.setHeader(
    'Content-Type',
    req.url === '/studio.js' ? 'text/javascript' : req.url === '/avatar.vrm' ? 'model/gltf-binary' : 'text/html',
  )
  res.end(req.url === '/studio.js' ? bundle.outputFiles[0].contents : req.url === '/avatar.vrm' ? model : html)
})
await new Promise<void>((done) => server.listen(0, '127.0.0.1', done))
const address = server.address()
if (!address || typeof address === 'string') throw new Error('Studio sem porta TCP')
const browser = await chromium.launch({ headless: true, ...(browserChannel ? { channel: browserChannel } : {}) })
const errors: string[] = []
try {
  const page = await browser.newPage({
    viewport: { width: 720, height: 900 },
    deviceScaleFactor: 1,
    ...(video ? { recordVideo: { dir: destination, size: { width: 720, height: 900 } } } : {}),
  })
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text())
  })
  await page.addInitScript('globalThis.__name = (fn) => fn')
  await page.goto(`http://127.0.0.1:${address.port}`)
  await page.waitForFunction(() => 'studio' in window)
  const actions = await page.evaluate(() => Object.keys((window as StudioWindow).studio.durations))
  const selected = process.argv
    .find((arg) => arg.startsWith('--poses='))
    ?.slice(8)
    .split(',')
  const poseActions = process.argv.includes('--still-only')
    ? []
    : actions.filter((name) => !selected || selected.includes(name))
  if (selected?.some((name) => !actions.includes(name))) throw new Error(`Pose desconhecida: ${selected.join(',')}`)
  const expressions = process.argv.includes('--expressions')
    ? ['face-happy', 'face-angry', 'face-sad', 'face-surprised', 'face-speaking', 'face-wink']
    : []
  const views = ['idle', ...poseActions, 'three-quarter', 'profile', 'back', 'feet', 'portrait', ...expressions]
  for (const action of views) {
    await page.evaluate((name) => {
      const studio = (window as StudioWindow).studio
      const { avatar, durations } = studio

      function frameAngledView(viewName: string) {
        const angle = viewName === 'profile' ? Math.PI / 2 : Math.PI / 4
        avatar.camera.position.set(Math.sin(angle) * 3.5, 0.9, Math.cos(angle) * 3.5)
        avatar.camera.lookAt(0, 0.85, 0)
      }

      function frameBack() {
        avatar.camera.position.set(0, 0.95, -3.4)
        avatar.camera.lookAt(0, 0.95, 0)
      }

      function frameFeet() {
        avatar.camera.position.set(0.35, 0.3, 0.9)
        avatar.camera.lookAt(0, 0.12, 0)
      }

      function frameFace(viewName: string) {
        avatar.camera.position.set(0, 1.28, 1.18)
        avatar.camera.lookAt(0, 1.24, 0)
        if (viewName === 'face-speaking') {
          studio.volume.current = 0.5
          studio.vowels.current = { a: 0.5, i: 0, u: 0, e: 0, o: 0 }
          avatar.setSpeaking(true)
        } else if (viewName === 'face-wink') avatar.playAction('wink')
        else if (viewName.startsWith('face-')) avatar.setExpression(viewName.slice(5), 0.7)
        if (viewName.startsWith('face-')) for (let i = 0; i < 48; i++) avatar.update(1 / 60)
      }

      function playPose(poseName: string) {
        avatar.playAction(poseName)
        for (let i = 0; i < Math.round(durations[poseName] * 0.5 * 60); i++) avatar.update(1 / 60)
      }

      avatar.setExpression('neutral', 0)
      studio.volume.current = 0
      studio.vowels.current = { a: 0, i: 0, u: 0, e: 0, o: 0 }
      avatar.setSpeaking(false)
      for (let i = 0; i < 300; i++) avatar.update(1 / 60)
      avatar.camera.position.set(0, 0.85, 3.6)
      avatar.camera.lookAt(0, 0.85, 0)
      // fitCamera define o near para o enquadramento inicial; estas vistas aproximam a câmera.
      avatar.camera.near = 0.05
      avatar.camera.far = 20
      avatar.camera.updateProjectionMatrix()
      if (name === 'three-quarter' || name === 'profile') frameAngledView(name)
      else if (name === 'back') frameBack()
      else if (name === 'feet') frameFeet()
      else if (name === 'portrait' || name.startsWith('face-')) frameFace(name)
      else if (name !== 'idle') playPose(name)
      avatar.render()
    }, action)
    await page.screenshot({ path: resolve(destination, `${action}.png`) })
    if (action === 'idle' && process.argv.includes('--thumbnail')) {
      await page.screenshot({ path: resolve('apps/desktop/src/renderer/public/vrm/mika/preview-haru.png') })
    }
  }
  if (video) {
    await page.evaluate(() => {
      const { avatar } = (window as StudioWindow).studio
      avatar.camera.position.set(0, 0.85, 3.6)
      avatar.camera.lookAt(0, 0.85, 0)
    })
    for (const name of poseActions) {
      await page.evaluate(async (action) => {
        const { avatar, durations } = (window as StudioWindow).studio
        avatar.playAction(action)
        let tick = performance.now()
        let elapsed = 0
        while (elapsed < durations[action] + 0.3) {
          await new Promise(requestAnimationFrame)
          const now = performance.now()
          const dt = (now - tick) / 1000
          // Subpassos preservam a física; a gravação conserva o tempo real mesmo com GPU por software.
          for (let remaining = dt; remaining > 0; remaining -= 1 / 60) avatar.update(Math.min(remaining, 1 / 60))
          elapsed += dt
          tick = now
          avatar.render()
        }
      }, name)
    }
  }
  if (!artOnly) {
    const stats = await page.evaluate(() => {
      const { avatar } = (window as StudioWindow).studio
      const canvas = document.querySelector('canvas')
      if (!canvas) throw new Error('Studio sem canvas')
      const image = new OffscreenCanvas(canvas.width, canvas.height)
      const context = image.getContext('2d')
      if (!context) throw new Error('Studio sem contexto para ler a imagem final')
      const autoReset = avatar.renderer.info.autoReset
      avatar.renderer.info.autoReset = false
      avatar.renderer.info.reset()
      avatar.render()
      // O post-FX usa um alvo MSAA e um quad: readPixels pode ler o framebuffer intermediário.
      // A cópia do canvas mede o resultado final que aparece também no PNG.
      context.drawImage(canvas, 0, 0)
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
      let visible = 0
      for (let i = 3; i < pixels.length; i += 4) if (pixels[i] > 0) visible++
      const stats = {
        visible,
        calls: avatar.renderer.info.render.calls,
        triangles: avatar.renderer.info.render.triangles,
      }
      avatar.renderer.info.autoReset = autoReset
      return stats
    })
    if (errors.length || stats.visible < 10_000) throw new Error(JSON.stringify({ errors, stats }))
    const report = {
      destination,
      modelPath,
      modelSha256: createHash('sha256').update(model).digest('hex'),
      actions,
      stats,
      errors,
    }
    await writeFile(resolve(destination, 'validation.json'), JSON.stringify(report, null, 2))
    process.stdout.write(`${JSON.stringify(report)}\n`)
  } else {
    process.stdout.write(`${JSON.stringify({ destination, views, errors })}\n`)
  }
  const recording = page.video()
  await page.close()
  if (recording) await recording.saveAs(resolve(destination, 'haru-motion-reel.webm'))
} finally {
  await browser.close()
  await new Promise<void>((done) => server.close(() => done()))
}
