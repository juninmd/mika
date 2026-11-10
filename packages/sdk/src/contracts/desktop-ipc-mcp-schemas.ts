import { z } from 'zod'
import { DESKTOP_INVOKE_CHANNELS } from './desktop-ipc-channels'
import { emptyPayloadSchema } from './desktop-ipc-core-schemas'

// Mesmos tetos de `apps/desktop/src-tauri/src/modules/mcp/editor.rs` — o Rust revalida; aqui é
// para a UI recusar antes de atravessar a ponte.
const mcpServerNameSchema = z
  .string()
  .trim()
  .min(1)
  .max(64)
  .regex(/^[A-Za-z0-9_.-]+$/)

/**
 * Nome de uma entrada que JÁ está no `mcp.json` (remover, editar sem renomear). O arquivo é do
 * usuário e pode ter nomes fora do alfabeto acima (`"brave search"`) — `editor::parse_existing_name`.
 */
const mcpExistingServerNameSchema = z.string().min(1).max(4096)

/**
 * Servidor como a UI o manda. `env` com valor `null` = "mantenha o valor salvo" — a lista nunca
 * devolve valores de env, só as chaves, então editar um servidor com segredo reenvia `null`.
 */
export const mcpServerSpecInputSchema = z.object({
  command: z.string().trim().min(1).max(1024),
  args: z.array(z.string().max(4096)).max(64).optional(),
  env: z.record(z.string().regex(/^[A-Za-z_][A-Za-z0-9_]{0,127}$/), z.string().max(8192).nullable()).optional(),
  /** Caminho absoluto; vazio = diretório herdado. */
  cwd: z.string().max(4096).nullable().optional(),
  /** Ausente = true. `false` grava `"disabled": true` no mcp.json. */
  enabled: z.boolean().optional(),
})

export const mcpServerToolSchema = z.object({
  name: z.string(),
  key: z.string().optional(),
  description: z.string().nullable().optional(),
})

export const mcpServerStatusSchema = z.enum(['connected', 'error', 'stopped'])

export const mcpServerSummarySchema = z.object({
  name: z.string(),
  command: z.string(),
  args: z.array(z.string()),
  envKeys: z.array(z.string()),
  /** Variáveis cujo valor salvo a `master.key` atual não decifra: seguem no arquivo, não vão ao servidor. */
  unreadableEnv: z.array(z.string()).optional(),
  cwd: z.string().nullable(),
  enabled: z.boolean(),
  status: mcpServerStatusSchema,
  error: z.string().optional(),
  tools: z.array(mcpServerToolSchema),
})

export const mcpServersListResponseSchema = z.object({
  configPath: z.string(),
  configDir: z.string(),
  /** `mcp.enabled` — desligado, nenhum servidor sobe (status `stopped`). */
  enabled: z.boolean(),
  offlineMode: z.boolean(),
  /** mcp.json ilegível: `servers` vem vazio e nada é gravado até o usuário corrigir. */
  error: z.string().nullable(),
  servers: z.array(mcpServerSummarySchema),
})

export const mcpServerSaveRequestSchema = z
  .object({
    name: z.string(),
    spec: mcpServerSpecInputSchema,
    /** Nome atual ao editar/renomear; ausente = criar (nome repetido é recusado). */
    originalName: mcpExistingServerNameSchema.optional(),
  })
  .superRefine((request, ctx) => {
    // Sem renomear, o nome salvo vale como está; nome novo segue o alfabeto de `editor::parse_name`.
    if (request.originalName !== undefined && request.name === request.originalName) return
    if (!mcpServerNameSchema.safeParse(request.name).success) {
      ctx.addIssue({ code: 'custom', path: ['name'], message: 'invalid MCP server name' })
    }
  })

const mcpFailureSchema = z.object({ ok: z.literal(false), error: z.string() })

export const mcpServerSaveResponseSchema = z.union([
  z.object({ ok: z.literal(true), server: mcpServerSummarySchema }),
  mcpFailureSchema,
])

export const mcpServerRemoveRequestSchema = z.object({ name: mcpExistingServerNameSchema })

export const mcpServerRemoveResponseSchema = z.union([z.object({ ok: z.literal(true) }), mcpFailureSchema])

export const mcpServerTestRequestSchema = z.object({
  spec: mcpServerSpecInputSchema,
  /** Resolve `env: null` contra a entrada salva com este nome. */
  originalName: mcpExistingServerNameSchema.optional(),
})

export const mcpServerTestResponseSchema = z.union([
  z.object({ ok: z.literal(true), tools: z.array(mcpServerToolSchema) }),
  mcpFailureSchema,
])

export const mcpOpenConfigDirResponseSchema = z.union([
  z.object({ ok: z.literal(true), path: z.string() }),
  mcpFailureSchema,
])

export type McpServerSpecInput = z.infer<typeof mcpServerSpecInputSchema>
export type McpServerSummary = z.infer<typeof mcpServerSummarySchema>
export type McpServersListResponse = z.infer<typeof mcpServersListResponseSchema>
export type McpServerSaveRequest = z.infer<typeof mcpServerSaveRequestSchema>
export type McpServerSaveResponse = z.infer<typeof mcpServerSaveResponseSchema>
export type McpServerRemoveResponse = z.infer<typeof mcpServerRemoveResponseSchema>
export type McpServerTestRequest = z.infer<typeof mcpServerTestRequestSchema>
export type McpServerTestResponse = z.infer<typeof mcpServerTestResponseSchema>
export type McpOpenConfigDirResponse = z.infer<typeof mcpOpenConfigDirResponseSchema>

export const DESKTOP_MCP_ROUTES = {
  mcpServersList: {
    channel: DESKTOP_INVOKE_CHANNELS.McpServersList,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: mcpServersListResponseSchema,
  },
  mcpServerSave: {
    channel: DESKTOP_INVOKE_CHANNELS.McpServerSave,
    namespace: 'runtime',
    requestSchema: mcpServerSaveRequestSchema,
    responseSchema: mcpServerSaveResponseSchema,
  },
  mcpServerRemove: {
    channel: DESKTOP_INVOKE_CHANNELS.McpServerRemove,
    namespace: 'runtime',
    requestSchema: mcpServerRemoveRequestSchema,
    responseSchema: mcpServerRemoveResponseSchema,
  },
  mcpServerTest: {
    channel: DESKTOP_INVOKE_CHANNELS.McpServerTest,
    namespace: 'runtime',
    requestSchema: mcpServerTestRequestSchema,
    responseSchema: mcpServerTestResponseSchema,
  },
  mcpOpenConfigDir: {
    channel: DESKTOP_INVOKE_CHANNELS.McpOpenConfigDir,
    namespace: 'runtime',
    requestSchema: emptyPayloadSchema,
    responseSchema: mcpOpenConfigDirResponseSchema,
  },
} as const
