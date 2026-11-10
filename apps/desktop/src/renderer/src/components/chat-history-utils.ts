export type ChatHistoryMessage = {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  /** Falada ou digitada. Ausente em mensagens gravadas antes desta coluna existir. */
  source?: 'voice' | 'text'
}

export function mergeHistoryMessages(
  remoteMessages: ChatHistoryMessage[],
  sessionMessages: ChatHistoryMessage[],
): ChatHistoryMessage[] {
  const seenIds = new Set<string>()
  const seenContent = new Set<string>()
  const merged: ChatHistoryMessage[] = []

  for (let i = 0; i < remoteMessages.length; i++) {
    const message = remoteMessages[i]
    const key = message.id || `${message.role}:${message.content}`
    if (!seenIds.has(key)) {
      seenIds.add(key)
      seenContent.add(`${message.role}:${message.content}`)
      merged.push(message)
    }
  }

  for (let i = 0; i < sessionMessages.length; i++) {
    const message = sessionMessages[i]
    const idKey = message.id || `${message.role}:${message.content}`
    const contentKey = `${message.role}:${message.content}`
    if (!seenIds.has(idKey) && !seenContent.has(contentKey)) {
      seenIds.add(idKey)
      seenContent.add(contentKey)
      merged.push(message)
    }
  }

  if (merged.length === remoteMessages.length) return remoteMessages

  return merged.sort((left, right) => left.timestamp - right.timestamp)
}
