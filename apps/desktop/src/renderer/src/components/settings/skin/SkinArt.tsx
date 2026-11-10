import { ImageOff } from 'lucide-react'
import { useState } from 'react'
import type { Live2DModelOption } from '../../live2d/model-path'

/** A skin's preview art, or its initial when the PNG is missing (Live2D samples are not packaged). */
export function SkinArt({ model, label, className }: { model: Live2DModelOption; label: string; className: string }) {
  const [broken, setBroken] = useState(false)
  if (!model.preview || broken) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-1 text-text-muted" aria-hidden="true">
        <span className="text-4xl font-black leading-none">{label.charAt(0)}</span>
        <ImageOff size={14} />
      </div>
    )
  }
  return (
    <img
      src={model.preview}
      alt=""
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => setBroken(true)}
      className={className}
    />
  )
}
