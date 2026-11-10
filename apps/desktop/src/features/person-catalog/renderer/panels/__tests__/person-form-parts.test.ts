import { describe, expect, it } from 'vitest'
import { buildSavePayload, initialSaveFields } from '../person-form-parts'

describe('person-form-parts — buildSavePayload', () => {
  it('drops a relationship row left blank by the user instead of saving an empty edge', () => {
    // Item 7 do pacote "people catalog + relationship graph": clicar "add" e não preencher não pode
    // virar uma aresta {name:"", type:""} no grafo — `tool.rs::parse_relationships` não filtra isso.
    const payload = buildSavePayload({
      name: 'João',
      type: 'real_person',
      bio: '',
      role: '',
      relationToUser: '',
      relationships: [
        { name: 'Maria', type: 'trabalha com' },
        { name: '', type: '' },
        { name: 'Pedro', type: '  ' },
      ],
      origin: '',
      nationality: '',
      imageUrl: '',
      userNotes: '',
      tagsStr: '',
    })
    expect(payload.relationships).toEqual([{ name: 'Maria', type: 'trabalha com' }])
  })

  it('trims name/type in a kept relationship row', () => {
    const payload = buildSavePayload({
      name: 'João',
      type: 'real_person',
      bio: '',
      role: '',
      relationToUser: '',
      relationships: [{ name: '  Maria  ', type: '  trabalha com  ' }],
      origin: '',
      nationality: '',
      imageUrl: '',
      userNotes: '',
      tagsStr: '',
    })
    expect(payload.relationships).toEqual([{ name: 'Maria', type: 'trabalha com' }])
  })

  it('sends relationToUser trimmed, and undefined (not an empty string) when blank', () => {
    const withValue = buildSavePayload({
      name: 'João',
      type: 'real_person',
      bio: '',
      role: '',
      relationToUser: '  irmão  ',
      relationships: [],
      origin: '',
      nationality: '',
      imageUrl: '',
      userNotes: '',
      tagsStr: '',
    })
    expect(withValue.relationToUser).toBe('irmão')

    const blank = buildSavePayload({
      name: 'João',
      type: 'real_person',
      bio: '',
      role: '',
      relationToUser: '   ',
      relationships: [],
      origin: '',
      nationality: '',
      imageUrl: '',
      userNotes: '',
      tagsStr: '',
    })
    expect(blank.relationToUser).toBeUndefined()
  })
})

describe('person-form-parts — initialSaveFields', () => {
  it('reads relationToUser/relationships back from an existing person record', () => {
    const fields = initialSaveFields({
      primaryName: 'João',
      relationToUser: 'irmão',
      relationships: [{ name: 'Maria', type: 'trabalha com' }],
    })
    expect(fields.relationToUser).toBe('irmão')
    expect(fields.relationships).toEqual([{ name: 'Maria', type: 'trabalha com' }])
  })

  it('defaults relationToUser/relationships when creating a new person (no initial record)', () => {
    const fields = initialSaveFields(undefined)
    expect(fields.relationToUser).toBe('')
    expect(fields.relationships).toEqual([])
  })
})
