export const API_CALL_ARGS: Record<string, unknown[]> = {
  'runtime.transcribeAudio': [
    { audioBase64: 'UklGRiQAAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQAAAAA=', mimeType: 'audio/wav' },
  ],
  'runtime.sendVadAudio': [{ samples: [0, 0, 0], amplitude: 0 }],
  'runtime.getLlmModels': [{ provider: 'ollama', kind: 'chat' }],
  'runtime.getElevenLabsVoices': [{ apiKey: 'chave-invalida-de-homologacao' }],
  'runtime.testPlugin': [{ pluginId: 'mika.mood' }],
  'runtime.executeTool': ['get_system_info', {}],
  'runtime.getToolsByPlugin': ['mika.mcp'],
  'runtime.reportChatResponse': ['homologação da superfície de API: relatório de teste'],
  'runtime.sendMicState': [false],
  'runtime.sendToolApprovalResponse': ['homologacao-sem-chamada-pendente', false, 'once'],
  'runtime.log': [{ level: 'debug', scope: 'homologate-api', msg: 'homologação da superfície de API' }],
  'runtime.resetProviderCircuit': ['ollama'],
  // `getLegalDocument(doc)` embrulha `{ doc }`: sem argumento o Zod rejeita `{ doc: undefined }`.
  'runtime.getLegalDocument': ['eula'],
  'runtime.checkSilenceCommand': [{ text: 'homologação da superfície de API' }],
  'runtime.getModelProfile': ['ollama', 'homologacao-modelo-inexistente'],
  'runtime.prepareWizardAsset': [{ assetId: 'vad' }],
  'runtime.savePrompt': [{ namespace: 'homologacao', key: 'inexistente', text: 'sem efeito' }],
  'runtime.resetPrompt': [{ namespace: 'homologacao', key: 'inexistente' }],
  'shell.window.setIgnoreMouseEvents': [false],
  'shell.window.moveBy': [0, 0],

  'shell.system.preventSleep': [false],
  'shell.pluginRpc': [{ pluginId: 'mika.mood', method: 'mood_get', args: {} }],
  'shell.pluginViews.open': [{ pluginId: 'mika.person-catalog', viewId: 'person-catalog.listing' }],
}

export const API_UNSAFE: string[] = [
  'runtime.quitAndInstall',
  'shell.window.close',
  'shell.window.startDrag',

  'runtime.clearChatHistory',
  'runtime.resetTokenUsage',

  'runtime.saveSetting',
  'runtime.setTelemetryOptIn',
  'shell.autoLaunch.set',

  'runtime.sendChatMessage',
  'runtime.checkForUpdates',
  'runtime.downloadUpdate',

  'shell.openSettings',

  // Efeito colateral real: encerra o app (`quit`), abre navegador/gerenciador de arquivos do SO,
  // muta `mcp.json`, sobe processo MCP, baixa modelo, chama LLM/rede. Cada um tem dono em spec própria
  // (tray/shutdown-persistence, settings-full-audit); chamar sem argumento só provaria a rejeição do Zod.
  'shell.system.quit',
  'shell.system.openExternal',
  'shell.system.revealPath',
  'runtime.saveMcpServer',
  'runtime.removeMcpServer',
  'runtime.testMcpServer',
  'runtime.pullLlmModel',
  'runtime.testCredential',
  'runtime.wizardReact',
]
