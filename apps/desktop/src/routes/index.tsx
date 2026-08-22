import { createFileRoute } from '@tanstack/react-router'
import { AppVersion } from '@/components/app-version'
import { Brand } from '@/components/brand'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  return (
    <section className="mx-auto flex max-w-3xl flex-col gap-6">
      <Brand />
      <AppVersion />
    </section>
  )
}
