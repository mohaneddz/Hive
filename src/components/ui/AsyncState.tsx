import { AlertTriangle, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/Button";

export function PageSkeleton({ cards = 8 }: { cards?: number }) {
  return (
    <div className="mt-7 animate-pulse" aria-label="Loading" role="status">
      <div className="h-4 w-40 rounded-full bg-ink/10" />
      <div className="mt-5 grid grid-cols-2 gap-4 lg:grid-cols-3 xl:grid-cols-4">
        {Array.from({ length: cards }, (_, index) => (
          <div key={index} className="aspect-square rounded-[20px] bg-ink/[.07]" />
        ))}
      </div>
      <span className="sr-only">Loading content…</span>
    </div>
  );
}

export function PageError({ message, onRetry, label = "Retry" }: { message: string; onRetry?: () => void; label?: string }) {
  return (
    <div className="mt-7 flex items-start justify-between gap-4 rounded-2xl border border-red-500/25 bg-red-500/5 p-4 text-red-600" role="alert">
      <div className="flex min-w-0 items-start gap-3">
        <AlertTriangle size={17} className="mt-0.5 shrink-0" />
        <div>
          <p className="text-sm font-extrabold">Couldn’t load this page</p>
          <p className="mt-1 break-words text-xs opacity-90">{message}</p>
        </div>
      </div>
      {onRetry && (
        <Button variant="secondary" icon={<RefreshCw size={14} />} onClick={onRetry} className="shrink-0">
          {label}
        </Button>
      )}
    </div>
  );
}
