import { z } from 'zod'

export const SlangRegisterSchema = z.enum(['cyberpunk', 'noir']).describe('Tonal register of the slang term')

export const SlangModeSchema = z
  .enum(['technical', 'combat', 'idle'])
  .describe('Active slang mode: technical (IDE/code), combat (gaming), idle (default)')
export type SlangMode = z.infer<typeof SlangModeSchema>

export const SlangEntrySchema = z.object({
  term: z.string().min(2).max(80).describe('The slang term or expression itself'),
  meaning: z.string().min(2).max(200).describe('When/how the term should be used'),
  register: SlangRegisterSchema.default('cyberpunk'),
})
export type SlangEntry = z.infer<typeof SlangEntrySchema>

export const SlangDictionarySchema = z
  .object({
    technical: z.array(SlangEntrySchema).default([]),
    combat: z.array(SlangEntrySchema).default([]),
    idle: z.array(SlangEntrySchema).default([]),
  })
  .describe('Slang dictionary partitioned by mode')
export type SlangDictionary = z.infer<typeof SlangDictionarySchema>

export const SlangSignalSchema = z.object({
  mode: SlangModeSchema,
  source: z.enum(['ide', 'game', 'window', 'idle']).describe('Which awareness signal selected the mode'),
  detail: z.string().max(120).optional().describe('Process name, window title or game name'),
})
export type SlangSignal = z.infer<typeof SlangSignalSchema>

export const DEFAULT_SLANG_DICTIONARY: SlangDictionary = {
  technical: [
    { term: 'fiação exposta', meaning: 'código fortemente acoplado, sem isolamento', register: 'noir' },
    { term: 'gambiarra de neon', meaning: 'workaround feio mas funcional', register: 'cyberpunk' },
    { term: 'beco molhado', meaning: 'caminho de código duvidoso, cheio de side-effects', register: 'noir' },
    { term: 'firewall furado', meaning: 'validação ausente em borda de sistema', register: 'cyberpunk' },
    { term: 'cabo solto', meaning: 'dependência implícita não declarada', register: 'noir' },
  ],
  combat: [
    {
      term: 'hackeado pelo mainframe',
      meaning: 'tomou dano absurdo / morreu de forma humilhante',
      register: 'cyberpunk',
    },
    { term: 'cano queimado', meaning: 'reload mal calculado, ficou sem munição na hora errada', register: 'noir' },
    { term: 'circuito frito', meaning: 'erro tático grave, paralisou na hora do combate', register: 'cyberpunk' },
    { term: 'sombra na esquina', meaning: 'emboscada óbvia que o usuário não viu chegar', register: 'noir' },
    { term: 'overclock burro', meaning: 'agressividade sem leitura, levou hit desnecessário', register: 'cyberpunk' },
  ],
  idle: [
    { term: 'rua deserta', meaning: 'silêncio prolongado, contexto parado', register: 'noir' },
    { term: 'standby de neon', meaning: 'esperando ação, energia contida', register: 'cyberpunk' },
  ],
}
