import type { Page } from '@playwright/test'

export type IpcHandlers = Record<string, (args: unknown) => unknown>

/**
 * Installs a minimal `window.__TAURI_INTERNALS__` before the app boots, mirroring @tauri-apps/api/mocks.
 * Handlers are serialized with `toString()` and rebuilt in the page: pass self-contained arrow functions
 * (no closures over test-scope variables, no method shorthand).
 */
export async function installTauriMock(page: Page, handlers: IpcHandlers) {
  await page.addInitScript(
    (serialized: string) => {
      // oxlint-disable-next-line typescript/no-implied-eval, typescript/no-unsafe-type-assertion -- handlers are serialized by our own test helper and rebuilt in the browser
      const table = new Function(`return (${serialized})`)() as Record<string, (args: unknown) => unknown>
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- window is widened to attach the Tauri internals global
      const w = window as unknown as Record<string, unknown>
      // oxlint-disable-next-line no-underscore-dangle -- global name is dictated by @tauri-apps/api
      w.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        transformCallback: (cb: (...a: unknown[]) => void) => {
          const id = Math.floor(Math.random() * 1e9)
          // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- callbacks live on window under Tauri's `_<id>` convention
          ;(window as unknown as Record<string, unknown>)[`_${id}`] = cb
          return id
        },
        unregisterCallback: (id: number) => {
          delete w[`_${id}`]
        },
        invoke: async (cmd: string, args: unknown) => {
          if (cmd === 'plugin:event|listen') return 1
          if (cmd === 'plugin:event|unlisten') return undefined
          const h = table[cmd]
          if (!h) throw new Error(`unmocked IPC command: ${cmd}`)
          return h(args)
        },
      }
      // oxlint-disable-next-line no-underscore-dangle -- Tauri runtime global
      w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    },
    `{${Object.entries(handlers)
      .map(([k, f]) => `${JSON.stringify(k)}: ${f.toString()}`)
      .join(',')}}`,
  )
}
