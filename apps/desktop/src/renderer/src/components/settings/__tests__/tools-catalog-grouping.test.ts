import { describe, expect, it } from 'vitest'
import type { ToolCatalogItem } from '../../../lib/shell-api'
import { groupByPlugin, PLUGIN_LABEL_KEYS } from '../tools-mcp-catalog'

const tool = (name: string, pluginId?: string) => ({ name, description: '', pluginId }) as ToolCatalogItem

describe('tool catalog grouping', () => {
  it('merges bare and prefixed ids of the same feature into one labelled group', () => {
    const groups = groupByPlugin([tool('voice', 'mika.voice'), tool('speak', 'voice'), tool('vision_read', 'vision')])
    expect(groups.map(([id, items]) => [id, items.length])).toEqual([
      ['mika.vision', 1],
      ['mika.voice', 2],
    ])
    for (const [id] of groups) expect(PLUGIN_LABEL_KEYS[id]).toBeDefined()
  })

  it('keeps tools without a plugin under core', () => {
    expect(groupByPlugin([tool('a'), tool('b', 'core')])).toEqual([['core', [tool('a'), tool('b', 'core')]]])
  })
})
