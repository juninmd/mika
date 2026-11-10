/**
 * Servidores MCP oficiais/populares prontos para preencher o formulário com um clique. Os pacotes
 * são os nomes reais publicados (npm / PyPI) — o usuário só completa o que cada um precisa.
 */
export type McpTemplateNeed = 'folder' | 'apiKey' | 'none' | 'python' | 'browser'

export type McpTemplate = {
  id: 'filesystem' | 'github' | 'brave-search' | 'fetch' | 'memory' | 'playwright'
  /** Marca/nome do servidor, igual em todo idioma. */
  brand: string
  /** Nome sugerido para a entrada no mcp.json. */
  serverName: string
  command: string
  args: string[]
  /** Chaves de env que o servidor exige (valor preenchido pelo usuário). */
  env: string[]
  /** Quantos args são obrigatórios antes de salvar (ex.: a pasta do filesystem). */
  minArgs?: number
  need: McpTemplateNeed
  docsUrl: string
}

export const MCP_TEMPLATES: readonly McpTemplate[] = [
  {
    id: 'filesystem',
    brand: 'Filesystem',
    serverName: 'filesystem',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem'],
    env: [],
    minArgs: 3,
    need: 'folder',
    docsUrl: 'https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem',
  },
  {
    id: 'github',
    brand: 'GitHub',
    serverName: 'github',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    env: ['GITHUB_PERSONAL_ACCESS_TOKEN'],
    need: 'apiKey',
    docsUrl: 'https://github.com/settings/tokens',
  },
  {
    id: 'brave-search',
    brand: 'Brave Search',
    serverName: 'brave-search',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-brave-search'],
    env: ['BRAVE_API_KEY'],
    need: 'apiKey',
    docsUrl: 'https://brave.com/search/api/',
  },
  {
    id: 'fetch',
    brand: 'Fetch',
    serverName: 'fetch',
    command: 'uvx',
    args: ['mcp-server-fetch'],
    env: [],
    need: 'python',
    docsUrl: 'https://docs.astral.sh/uv/getting-started/installation/',
  },
  {
    id: 'memory',
    brand: 'Memory',
    serverName: 'memory',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-memory'],
    env: [],
    need: 'none',
    docsUrl: 'https://github.com/modelcontextprotocol/servers/tree/main/src/memory',
  },
  {
    id: 'playwright',
    brand: 'Playwright',
    serverName: 'playwright',
    command: 'npx',
    args: ['-y', '@playwright/mcp@latest'],
    env: [],
    need: 'browser',
    docsUrl: 'https://github.com/microsoft/playwright-mcp',
  },
]

/** Pacote do template sem a tag de versão (`@playwright/mcp@latest` → `@playwright/mcp`). */
const packageOf = (template: McpTemplate): string =>
  (template.args.find((arg) => !arg.startsWith('-')) ?? '').replace(/(.)@[^@/]*$/, '$1')

/** Algum servidor já roda este pacote (qualquer nome, qualquer versão)? */
export function templateInstalled(template: McpTemplate, servers: readonly { args: readonly string[] }[]): boolean {
  const pkg = packageOf(template)
  return servers.some((s) => s.args.some((arg) => arg === pkg || arg.startsWith(`${pkg}@`)))
}
