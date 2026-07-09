import type { ButtonHTMLAttributes } from "react";
import { cn } from "../utils/cn";

type Variant = "primary" | "ghost";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

const styles: Record<Variant, string> = {
  primary:
    "bg-blue-600 text-white hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-400",
  ghost:
    "bg-neutral-800 text-neutral-200 hover:bg-neutral-700 disabled:opacity-50",
};

export function Button({ variant = "primary", className, ...props }: ButtonProps) {
  return (
    <button
      className={cn(
        "rounded-md px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed",
        styles[variant],
        className,
      )}
      {...props}
    />
  );
}
