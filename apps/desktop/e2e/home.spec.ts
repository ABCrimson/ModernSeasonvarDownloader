import { expect, test } from '@playwright/test'
import { installTauriMock } from './tauri-mock'

test('home shows the brand and the version from the Rust side', async ({ page }) => {
  await installTauriMock(page, { app_version: () => '0.1.0-e2e' })
  await page.goto('/')
  await expect(page.getByRole('heading', { level: 1, name: 'Seasonvar Downloader' })).toBeVisible()
  await expect(page.getByText('v0.1.0-e2e')).toBeVisible()
})
