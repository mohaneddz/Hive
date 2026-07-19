import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@/utils/cn";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: ReactNode;
  variant?: "primary" | "secondary" | "ghost";
}

export function Button({
  children,
  className,
  icon,
  variant = "primary",
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex h-10 items-center justify-center gap-2 rounded-xl px-4 text-sm font-bold transition focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-honey/25 disabled:pointer-events-none disabled:opacity-50",
        variant === "primary" &&
          "bg-honey text-ink shadow-[0_8px_20px_rgba(227,161,5,.2)] hover:-translate-y-px hover:bg-honey-dark",
        variant === "secondary" &&
          "border border-ink/10 bg-panel text-ink hover:border-honey/40 hover:bg-cream/45",
        variant === "ghost" && "text-ink-soft hover:bg-ink/5 hover:text-ink",
        className,
      )}
      {...props}
    >
      {icon}
      {children}
    </button>
  );
}
