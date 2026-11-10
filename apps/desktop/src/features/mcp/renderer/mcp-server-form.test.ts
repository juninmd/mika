import type { McpServerSummary } from '@mika/sdk/contracts'
import { describe, expect, it } from 'vitest'
import {
  commandPreview,
  emptyForm,
  errorHint,
  formFromServer,
  formFromTemplate,
  type McpServerForm,
  pasteCommandLine,
  specFromForm,
  splitCommandLine,
  toggledSpec,
  validateForm,
} from './mcp-server-form'
import { MCP_TEMPLATES, templateInstalled } from './mcp-templates'

const server = (over: Partial<McpServerSummary> = {}): McpServerSummary => ({
  name: 'github',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-github'],
  envKeys: ['GITHUB_PERSONAL_ACCESS_TOKEN'],
  cwd: null,
  enabled: true,
  status: 'connected',
  tools: [],
  ...over,
})

const form = (over: Partial<McpServerForm> = {}): McpServerForm => ({ ...emptyForm(), ...over })
const envRow = (key: string, value: string, saved = false) => ({ id: key, key, value, saved })

describe('env secrets round-trip', () => {
  // The list never returns env values: editing a server and saving without touching a secret must
  // send `null` ("keep"), or the save would overwrite the token with an empty string.
  it('keeps an untouched saved secret as null', () => {
    const spec = specFromForm(formFromServer(server()))
    expect(spec.env).toEqual({ GITHUB_PERSONAL_ACCESS_TOKEN: null })
  })

  it('sends a retyped secret as its new value', () => {
    const f = formFromServer(server())
    const row = f.env[0]
    if (!row) throw new Error('row')
    const spec = specFromForm({ ...f, env: [{ ...row, value: 'ghp_new' }] })
    expect(spec.env).toEqual({ GITHUB_PERSONAL_ACCESS_TOKEN: 'ghp_new' })
  })

  it('drops blank rows and a removed saved key is simply absent', () => {
    const spec = specFromForm(form({ command: 'x', env: [envRow('', ''), envRow('A', '1')] }))
    expect(spec.env).toEqual({ A: '1' })
  })

  it('toggling a server keeps every saved secret and flips only enabled', () => {
    expect(toggledSpec(server(), false)).toEqual({
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      env: { GITHUB_PERSONAL_ACCESS_TOKEN: null },
      cwd: null,
      enabled: false,
    })
  })
})

describe('specFromForm', () => {
  it('trims command/cwd, maps blank cwd to null and drops empty args', () => {
    const spec = specFromForm(form({ command: '  uvx ', args: ['mcp-server-fetch', ''], cwd: '  ' }))
    expect(spec).toEqual({ command: 'uvx', args: ['mcp-server-fetch'], env: {}, cwd: null, enabled: true })
  })
})

describe('splitCommandLine / pasteCommandLine', () => {
  it.each([
    ['npx -y @modelcontextprotocol/server-memory', ['npx', '-y', '@modelcontextprotocol/server-memory']],
    ['npx -y pkg "/home/me/My Projects"', ['npx', '-y', 'pkg', '/home/me/My Projects']],
    ["uvx   mcp-server-fetch  '--user agent'", ['uvx', 'mcp-server-fetch', '--user agent']],
    ['C:\\tools\\srv.exe --root C:\\dev', ['C:\\tools\\srv.exe', '--root', 'C:\\dev']],
    ['   ', []],
  ])('%s', (line, parts) => {
    expect(splitCommandLine(line)).toEqual(parts)
  })

  // Users paste the README one-liner into "command": it has to become command + args, not a
  // command named "npx -y pkg" that fails to spawn.
  it('splits a pasted one-liner into command + args, appending to existing args', () => {
    expect(pasteCommandLine(form({ args: ['--x'] }), 'npx -y @playwright/mcp@latest')).toMatchObject({
      command: 'npx',
      args: ['-y', '@playwright/mcp@latest', '--x'],
    })
  })
})

describe('commandPreview', () => {
  it('quotes args with spaces', () => {
    expect(commandPreview('npx', ['-y', 'pkg', '/a b'])).toBe('npx -y pkg "/a b"')
  })
})

describe('validateForm', () => {
  const ok = form({ name: 'fetch', command: 'uvx', args: ['mcp-server-fetch'] })

  it('accepts a valid form', () => {
    expect(validateForm(ok, [])).toEqual({})
  })

  it.each<[string, Partial<McpServerForm>, string]>([
    ['name', { name: '' }, 'required'],
    ['name', { name: 'my server' }, 'nameChars'],
    ['name', { name: 'x'.repeat(65) }, 'nameLength'],
    ['command', { command: '  ' }, 'required'],
    ['cwd', { cwd: 'relative/dir' }, 'cwdAbsolute'],
    ['args', { minArgs: 2 }, 'argsMissing'],
  ])('%s → %s', (field, over, code) => {
    expect(validateForm({ ...ok, ...over }, [])[field]).toBe(code)
  })

  it.each(['/home/me', 'C:\\dev', 'D:/games', '\\\\nas\\share'])('accepts absolute cwd %s', (cwd) => {
    expect(validateForm({ ...ok, cwd }, []).cwd).toBeUndefined()
  })

  // Rust `ensure_name_is_free` compares sanitized names: "my-srv" collides with "my_srv".
  it('rejects a name equivalent to another server, but not to itself when editing', () => {
    expect(validateForm({ ...ok, name: 'my-srv' }, ['my_srv']).name).toBe('nameTaken')
    expect(validateForm({ ...ok, name: 'my-srv' }, ['my_srv'], 'my_srv').name).toBeUndefined()
  })

  it('flags bad, duplicated and empty env rows by row id', () => {
    const errors = validateForm(
      { ...ok, env: [envRow('1BAD', 'v'), envRow('K', 'a'), { ...envRow('K', 'b'), id: 'k2' }, envRow('E', '')] },
      [],
    )
    expect(errors).toEqual({ 'env.1BAD': 'envKey', 'env.k2': 'envDuplicate', 'env.E': 'envValue' })
  })

  it('a saved secret left blank is valid (kept)', () => {
    expect(validateForm({ ...ok, env: [envRow('TOKEN', '', true)] }, [])).toEqual({})
  })
})

describe('templates', () => {
  it('prefill command, args and the env keys each one needs', () => {
    const gh = MCP_TEMPLATES.find((t) => t.id === 'github')
    if (!gh) throw new Error('github template')
    const f = formFromTemplate(gh, [])
    expect(f).toMatchObject({ name: 'github', command: 'npx', args: ['-y', '@modelcontextprotocol/server-github'] })
    expect(f.env.map((r) => [r.key, r.value, r.saved])).toEqual([['GITHUB_PERSONAL_ACCESS_TOKEN', '', false]])
  })

  it('picks a free name when the template name is taken', () => {
    const mem = MCP_TEMPLATES.find((t) => t.id === 'memory')
    if (!mem) throw new Error('memory template')
    expect(formFromTemplate(mem, ['memory', 'memory-2']).name).toBe('memory-3')
  })

  it('filesystem needs a folder argument before it validates', () => {
    const fs = MCP_TEMPLATES.find((t) => t.id === 'filesystem')
    if (!fs) throw new Error('filesystem template')
    const f = formFromTemplate(fs, [])
    expect(validateForm(f, []).args).toBe('argsMissing')
    expect(validateForm({ ...f, args: [...f.args, '/home/me/code'] }, []).args).toBeUndefined()
  })

  it('every template validates once its needs are filled', () => {
    for (const tpl of MCP_TEMPLATES) {
      const f = formFromTemplate(tpl, [])
      const filled = {
        ...f,
        args: tpl.minArgs ? [...f.args, '/tmp/x'] : f.args,
        env: f.env.map((r) => ({ ...r, value: 'secret' })),
      }
      expect(validateForm(filled, []), tpl.id).toEqual({})
    }
  })
})

describe('errorHint', () => {
  it.each([
    ["command 'npx' not on PATH: No such file", 'npx', 'notFoundNode'],
    ["command 'uvx' not on PATH: x", 'C:\\bin\\uvx.exe', 'notFoundUv'],
    ["command 'srv' not on PATH: x", 'srv', 'notFound'],
    ['no answer in 20s', 'npx', 'timeout'],
    ['env TOKEN sem valor salvo para manter', 'npx', 'env'],
    ['Modo offline ligado: nenhum servidor MCP sobe', 'npx', 'offline'],
    ['spawn failed: permission denied', 'npx', 'generic'],
  ])('%s → %s', (message, command, hint) => {
    expect(errorHint(message, command)).toBe(hint)
  })
})

describe('templateInstalled', () => {
  const tpl = (id: string) => {
    const found = MCP_TEMPLATES.find((t) => t.id === id)
    if (!found) throw new Error(id)
    return found
  }
  it('matches the package under any server name and version tag', () => {
    expect(templateInstalled(tpl('playwright'), [{ args: ['@playwright/mcp@0.0.40'] }])).toBe(true)
    expect(templateInstalled(tpl('github'), [{ args: ['-y', '@modelcontextprotocol/server-github'] }])).toBe(true)
    expect(templateInstalled(tpl('fetch'), [{ args: ['mcp-server-fetch'] }])).toBe(true)
  })
  it('does not confuse sibling packages', () => {
    expect(templateInstalled(tpl('memory'), [{ args: ['-y', '@modelcontextprotocol/server-memory-plus'] }])).toBe(false)
  })
})
