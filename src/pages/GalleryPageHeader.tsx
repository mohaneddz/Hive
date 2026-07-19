import type { ReactNode } from "react";

export function GalleryPageHeader({ eyebrow, title, description, action }: { eyebrow: string; title: string; description: string; action?: ReactNode }) {
  return <header className="flex items-end justify-between gap-5"><div><p className="eyebrow">{eyebrow}</p><h1 className="mt-2 text-[30px] font-extrabold tracking-[-.04em] text-ink">{title}</h1><p className="mt-1.5 text-sm text-ink-muted">{description}</p></div>{action}</header>;
}
