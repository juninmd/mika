// Campos sujos (draft ≠ savedDraft) se registram aqui enquanto pendentes, para que o fechamento
// da janela (SettingsApp.tsx, beforeunload) force o mesmo `commit()` já usado no blur.
export const pendingCommits = new Map<string, () => Promise<void>>()

export function flushPendingSettingsCommits(): Promise<void> {
  const commits = Array.from(pendingCommits.values())
  return Promise.all(commits.map((run) => run().catch(() => undefined))).then(() => undefined)
}
