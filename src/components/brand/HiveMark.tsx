import { cn } from "@/utils/cn";

export function HiveMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 1024 1024"
      className={cn("size-8 text-honey", className)}
      aria-hidden="true"
    >
      <g fill="none" stroke="currentColor" strokeWidth="30" strokeLinejoin="round">
        <path d="M512 128 674 222 674 409 512 503 350 409 350 222Z" />
        <path d="M328 447 490 541 490 728 328 822 166 728 166 541Z" />
        <path d="M696 447 858 541 858 728 696 822 534 728 534 541Z" />
      </g>
    </svg>
  );
}
