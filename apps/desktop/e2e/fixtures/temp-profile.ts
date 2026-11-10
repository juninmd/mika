import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

export type TempProfile = {
  userDataDir: string
  cleanup: () => Promise<void>
}

async function removeWithRetry(dir: string, attempts = 6): Promise<void> {
  for (let attempt = 1; attempt <= attempts; attempt++) {
    try {
      await rm(dir, { recursive: true, force: true, maxRetries: 3, retryDelay: 150 })
      return
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code
      if (code !== 'EBUSY' && code !== 'EPERM' && code !== 'ENOTEMPTY') return
      if (attempt === attempts) return
      await new Promise((resolve) => setTimeout(resolve, 250 * attempt))
    }
  }
}

export async function createTempProfile(): Promise<TempProfile> {
  const userDataDir = await mkdtemp(join(tmpdir(), 'mika-e2e-'))
  return {
    userDataDir,
    cleanup: () => removeWithRetry(userDataDir),
  }
}
