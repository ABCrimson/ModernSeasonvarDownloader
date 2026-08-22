import { QueryClientProvider } from '@tanstack/react-query'
import { createRootRoute, Outlet } from '@tanstack/react-router'
import { queryClient } from '@/lib/query'

export const Route = createRootRoute({
  component: () => (
    <QueryClientProvider client={queryClient}>
      <main className="min-h-dvh bg-background p-8 text-foreground">
        <Outlet />
      </main>
    </QueryClientProvider>
  ),
})
