import { describe, expect, it, vi } from 'vitest'
import { discardIfCancelled, type Live2DModelInstance } from '../live2d-loader'

const fakeModel = () => {
  const destroy = vi.fn()
  return { model: { destroy } as unknown as Live2DModelInstance, destroy }
}

describe('discardIfCancelled', () => {
  // Regression: the stage could unmount while Live2DModel.from() was in flight; the model then resolved into
  // an already destroyed stage and its textures/internal model were never released.
  it('destroys the model with its textures when the stage went away while it loaded', () => {
    const { model, destroy } = fakeModel()
    expect(discardIfCancelled(model, () => true)).toBe(true)
    expect(destroy).toHaveBeenCalledExactlyOnceWith({ children: true, texture: true, baseTexture: true })
  })

  it('keeps the model when the stage is still mounted', () => {
    const { model, destroy } = fakeModel()
    expect(discardIfCancelled(model, () => false)).toBe(false)
    expect(destroy).not.toHaveBeenCalled()
  })
})
