export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  return String(error)
}

export type ElevenVoice = {
  voice_id: string
  name: string
  category: string
  labels: { gender?: string; language?: string; accent?: string; use_case?: string }
  preview_url: string

  availability?: 'included' | 'plan-restricted' | 'unknown'
  availabilityLabel?: string
}

export type Gender = 'all' | 'female' | 'male'
