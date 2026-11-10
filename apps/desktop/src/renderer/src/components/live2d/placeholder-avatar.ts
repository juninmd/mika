import '../../lib/pixi-unsafe-eval'
import * as PIXI from 'pixi.js'

export function showPlaceholder(app: PIXI.Application, message?: string): void {
  const avatar = new PIXI.Graphics()
  avatar.beginFill(0x2a1f4e, 0.8)
  avatar.drawCircle(0, 0, 80)
  avatar.endFill()
  avatar.beginFill(0x4a3f7e, 0.6)
  avatar.drawCircle(0, -15, 50)
  avatar.endFill()
  avatar.x = app.screen.width / 2
  avatar.y = app.screen.height * 0.45
  app.stage.addChild(avatar)

  const leftEye = new PIXI.Graphics()
  leftEye.beginFill(0x88ccff)
  leftEye.drawEllipse(0, 0, 8, 10)
  leftEye.endFill()
  leftEye.x = avatar.x - 18
  leftEye.y = avatar.y - 25
  app.stage.addChild(leftEye)

  const rightEye = new PIXI.Graphics()
  rightEye.beginFill(0x88ccff)
  rightEye.drawEllipse(0, 0, 8, 10)
  rightEye.endFill()
  rightEye.x = avatar.x + 18
  rightEye.y = avatar.y - 25
  app.stage.addChild(rightEye)

  const smile = new PIXI.Graphics()
  smile.lineStyle(2, 0xffaacc, 0.8)
  smile.arc(0, 0, 15, 0.2, Math.PI - 0.2)
  smile.x = avatar.x
  smile.y = avatar.y - 5
  app.stage.addChild(smile)

  const label = new PIXI.Text('✨ Mika ✨', {
    fontSize: 20,
    fill: 0xb0c4ff,
    fontFamily: 'Inter, sans-serif',
    fontWeight: 'bold',
  })
  label.anchor.set(0.5)
  label.x = app.screen.width / 2
  label.y = app.screen.height * 0.58
  app.stage.addChild(label)

  const hint = new PIXI.Text('Clique em ⚙️ para configurar', {
    fontSize: 12,
    fill: 0x7788bb,
    fontFamily: 'Inter, sans-serif',
  })
  hint.anchor.set(0.5)
  hint.x = app.screen.width / 2
  hint.y = app.screen.height * 0.63
  app.stage.addChild(hint)

  if (message) {
    const errorText = new PIXI.Text(`⚠️ ${message}`, {
      fontSize: 14,
      fill: 0xff5555,
      fontFamily: 'Inter, sans-serif',
      fontWeight: 'bold',
      align: 'center',
      wordWrap: true,
      wordWrapWidth: 350,
    })
    errorText.anchor.set(0.5)
    errorText.x = app.screen.width / 2
    errorText.y = app.screen.height * 0.75
    app.stage.addChild(errorText)
  }
}
