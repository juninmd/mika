import { type ClassValue, clsx } from 'clsx'
import { extendTailwindMerge } from 'tailwind-merge'

// Custom `--text-*` sizes from the app's @theme; unregistered, tailwind-merge reads `text-micro` as a
// colour and drops it next to `text-ok`, so the pill silently inherits its parent's font size.
const twMerge = extendTailwindMerge({ extend: { theme: { text: ['micro', 'caption'] } } })

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
