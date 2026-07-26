import type { MediaItem, TimelineGranularity } from "@/types/media";

export interface MediaGroup {
  key: string;
  label: string;
  /** Populated for every granularity so the year scrubber can find a section by year alone. */
  year: string | null;
  items: MediaItem[];
}

function itemDate(item: MediaItem): Date | null {
  const raw = item.takenAtOverride ?? item.takenAt ?? item.createdAt;
  const date = new Date(raw);
  return Number.isNaN(date.getTime()) ? null : date;
}

function isSameDay(a: Date, b: Date): boolean {
  return a.toDateString() === b.toDateString();
}

function dayLabel(date: Date): string {
  const now = new Date();
  if (isSameDay(date, now)) return "Today";
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) return "Yesterday";
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString(undefined, sameYear ? { month: "long", day: "numeric" } : { month: "long", day: "numeric", year: "numeric" });
}

function monthLabel(date: Date): string {
  return date.toLocaleDateString(undefined, { month: "long", year: "numeric" });
}

/** Buckets pre-sorted media (newest or oldest first) into date-header groups. */
export function groupByDate(items: MediaItem[], granularity: TimelineGranularity): MediaGroup[] {
  const groups: MediaGroup[] = [];
  const byKey = new Map<string, MediaGroup>();

  for (const item of items) {
    const date = itemDate(item);
    const key = !date
      ? "unknown"
      : granularity === "year"
        ? `${date.getFullYear()}`
        : granularity === "month"
          ? `${date.getFullYear()}-${date.getMonth()}`
          : date.toDateString();

    let group = byKey.get(key);
    if (!group) {
      const label = !date
        ? "Unknown date"
        : granularity === "year"
          ? `${date.getFullYear()}`
          : granularity === "month"
            ? monthLabel(date)
            : dayLabel(date);
      group = { key, label, year: date ? `${date.getFullYear()}` : null, items: [] };
      byKey.set(key, group);
      groups.push(group);
    }
    group.items.push(item);
  }

  return groups;
}
