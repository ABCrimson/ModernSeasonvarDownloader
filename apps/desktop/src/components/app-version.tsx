import { useQuery } from '@tanstack/react-query'
import { commands } from '@/bindings'

export function AppVersion() {
  const { data, error, isPending } = useQuery({
    queryKey: ['app', 'version'],
    queryFn: () => commands.appVersion(),
  })
  if (isPending) return <span className="text-xs text-muted-foreground">…</span>
  if (error) return <span className="text-xs text-destructive">version unavailable</span>
  return <span className="font-mono text-xs text-muted-foreground">v{data}</span>
}
