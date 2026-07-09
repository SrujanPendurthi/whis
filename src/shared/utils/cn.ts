/**
 * Minimal className combiner — joins truthy class fragments with a space.
 * Kept dependency-free; swap for `clsx`/`tailwind-merge` if needs grow.
 */
export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
