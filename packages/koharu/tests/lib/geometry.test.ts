import { describe, expect, it } from 'vitest'

import { resizeFrame, rotateFrame, selectableLayer, selectableTextLayer } from '@/lib/geometry'
import type { Frame } from '@koharu/bridge/protocol'

const frame: Frame = { x: 10, y: 20, width: 100, height: 50, angle_degrees: 0 }

describe('selection control geometry', () => {
  it('resizes from a handle while keeping the opposite edge fixed', () => {
    expect(resizeFrame(frame, 'e', { x: 140, y: 45 }, 1)).toEqual({
      ...frame,
      width: 130,
    })
    expect(resizeFrame(frame, 'nw', { x: 0, y: 10 }, 1)).toEqual({
      ...frame,
      x: 0,
      y: 10,
      width: 110,
      height: 60,
    })
  })

  it('resizes in the frame local axes when it is rotated', () => {
    const result = resizeFrame({ ...frame, angle_degrees: 90 }, 'e', { x: 60, y: 125 }, 1)
    expect(result.x).toBeCloseTo(-5)
    expect(result.y).toBeCloseTo(35)
    expect(result.width).toBeCloseTo(130)
    expect(result.height).toBe(50)
  })

  it('rotates around the frame center', () => {
    expect(rotateFrame(frame, { x: 60, y: -5 }, { x: 110, y: 45 })).toEqual({
      ...frame,
      angle_degrees: 90,
    })
  })
})

describe('layer selection helpers', () => {
  it('identifies selectable layers and distinguishes text layers for bulk selection', () => {
    const textLayer = { id: 'text-1', type: 'text' } as any
    const imageLayer = { id: 'img-1', type: 'image' } as any
    const artworkLayer = { id: 'art-1', type: 'artwork' } as any

    expect(selectableLayer(textLayer)).toBe(true)
    expect(selectableLayer(imageLayer)).toBe(true)
    expect(selectableLayer(artworkLayer)).toBe(false)

    expect(selectableTextLayer(textLayer)).toBe(true)
    expect(selectableTextLayer(imageLayer)).toBe(false)
    expect(selectableTextLayer(artworkLayer)).toBe(false)
  })
})
