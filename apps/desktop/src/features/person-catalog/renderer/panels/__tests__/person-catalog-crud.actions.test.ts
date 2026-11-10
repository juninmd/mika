import { describe, expect, it, vi } from 'vitest'
import type { PersonRecord } from '../../person-catalog.types'
import { buildCrudActions, type CrudActionDeps } from '../person-catalog-crud.actions'

type Call = { method: string; args: Record<string, unknown> }

function setup(episodicRemoved: number) {
  const calls: Call[] = []
  const setPendingDelete = vi.fn()
  const setMode = vi.fn()
  const rpc = {
    call: async (method: string, args: Record<string, unknown>) => {
      calls.push({ method, args })
      return { success: true, data: { episodicRemoved } }
    },
  }
  const deps = {
    api: { rpc },
    t: (key: string) => key,
    editing: { id: 'pc_1', primaryName: 'Link' } as PersonRecord,
    setEditing: vi.fn(),
    setMode,
    load: vi.fn(async () => {}),
    setPendingDelete,
  } as unknown as CrudActionDeps
  return { actions: buildCrudActions(deps), calls, setPendingDelete, setMode }
}

describe('person catalog — delete flow', () => {
  it('previews the memories it would erase and deletes nothing until the user confirms', async () => {
    const { actions, calls, setPendingDelete, setMode } = setup(7)

    await actions.handleDelete()

    expect(calls).toEqual([{ method: 'person_catalog_delete', args: { id: 'pc_1', dryRun: true } }])
    expect(setPendingDelete).toHaveBeenCalledWith({ episodic: 7 })
    expect(setMode).not.toHaveBeenCalled()
  })

  it('deletes right away when no memory mentions the person', async () => {
    const { actions, calls, setMode } = setup(0)

    await actions.handleDelete()

    expect(calls.map((c) => c.args)).toEqual([
      { id: 'pc_1', dryRun: true },
      { id: 'pc_1', confirm: true },
    ])
    expect(setMode).toHaveBeenCalledWith('list')
  })

  it('erases with an explicit confirmation after the user accepted the preview', async () => {
    const { actions, calls, setPendingDelete } = setup(7)

    await actions.confirmDelete()

    expect(calls).toEqual([{ method: 'person_catalog_delete', args: { id: 'pc_1', confirm: true } }])
    expect(setPendingDelete).toHaveBeenCalledWith(null)
  })
})
