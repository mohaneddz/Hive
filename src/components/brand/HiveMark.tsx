import { cn } from "@/utils/cn";

export function HiveMark({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "relative grid size-8 place-items-center rounded-[10px] bg-honey text-ink shadow-[0_6px_14px_rgba(227,161,5,.2)]",
        className,
      )}
      aria-hidden="true"
    >
      <svg viewBox="0 0 32 32" className="size-5 fill-current">
        <path d="M16 2.5 27.7 9v14L16 29.5 4.3 23V9L16 2.5Zm0 5.1-7.2 4v8.8l7.2 4 7.2-4v-8.8l-7.2-4Zm0 3.8 3.8 2.1v4.9L16 20.6l-3.8-2.2v-4.9l3.8-2.1Z" />
      </svg>
    </div>
  );
}
