import { callE2eHook, expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'

// Ported off `app.evaluate` (Electron main-process DI resolution + `app.quit()`). Tauri equivalents:
// - `add-brain-goal`/`list-brain-goals` call the REAL `brain::reflection::goals::add_goal` /
//   `get_active_goals` against the process' one `MemoryStore` (same SQLite file a real reflection
//   would write to) — see `src-tauri/src/modules/e2e_hooks.rs`.
// - `quit` runs the exact tray "Sair" sequence (`tray::mark_quitting` + `app.exit(0)`), which fires
//   `RunEvent::ExitRequested` in `lib.rs` and runs the real shutdown hooks before the process dies.
// - `app.process()` now returns the full `ChildProcess` (stdout/stderr streams); `app.waitForEvent
//   ('close')` resolves on the real process exit.
// `@libsql/client` is no longer a dependency (removed with the sidecar) — persistence is proven by
// reading the goal back through the SAME `MemoryStore` before quitting (the only opener of that
// SQLite file in this process) rather than a second, unavailable, out-of-process reader.
test('graceful quit persists a newly added goal and shuts down without core errors', async ({ app }) => {
  const output: string[] = []
  const child = app.process()
  child.stdout?.on('data', (chunk) => output.push(String(chunk)))
  child.stderr?.on('data', (chunk) => output.push(String(chunk)))

  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const goalId = 'shutdown-regression'
  await callE2eHook(page, 'add-brain-goal', {
    id: goalId,
    description: 'Persistir antes de fechar o banco',
    priority: 0.9,
  })

  const goals = await callE2eHook<Array<{ id: string; description: string }>>(page, 'list-brain-goals')
  const persisted = goals.find((g) => g.id === goalId)
  expect(persisted, `meta '${goalId}' não foi gravada antes do quit`).toBeTruthy()
  expect(persisted?.description).toBe('Persistir antes de fechar o banco')

  const closed = app.waitForEvent('close')
  await callE2eHook(page, 'quit')
  await closed

  expect(output.join('')).not.toMatch(/CLIENT_CLOSED|core shutdown failed|panicked at/)
  writeArtifact('shutdown-persistence', { goalId, persisted, processClosed: true, shutdownErrors: 0 })
})
