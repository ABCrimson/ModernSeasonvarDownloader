import { expect, test } from 'vitest'
import { render } from 'vitest-browser-react'
import { Brand } from './brand'

test('renders the product name as the page heading', async () => {
  const screen = await render(<Brand />)
  await expect.element(screen.getByRole('heading', { level: 1, name: 'Seasonvar Downloader' })).toBeVisible()
})
