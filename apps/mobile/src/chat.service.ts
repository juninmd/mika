import { invoke } from '@tauri-apps/api/core'

export interface ChatTurn {
  id: number
  role: 'user' | 'assistant'
  content: string
}

export interface LlmEndpoint {
  baseUrl: string
  model: string
  apiKey: string
}

/** Sends the conversation to the Rust backend, which calls the OpenAI-compatible endpoint. */
export function sendChat(endpoint: LlmEndpoint, turns: ChatTurn[]): Promise<string> {
  return invoke<string>('chat', { endpoint, turns })
}
