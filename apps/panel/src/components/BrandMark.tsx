import { cn } from "@/lib/utils";

interface BrandMarkProps {
  className?: string;
}

export function BrandMark({ className }: BrandMarkProps) {
  return (
    <svg
      viewBox="0 0 40 40"
      className={cn("h-8 w-8 shrink-0", className)}
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id="brand-mark-gradient" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="oklch(0.78 0.11 186)" />
          <stop offset="55%" stopColor="oklch(0.6 0.13 192)" />
          <stop offset="100%" stopColor="oklch(0.45 0.11 200)" />
        </linearGradient>
      </defs>
      <rect width="40" height="40" rx="11" fill="url(#brand-mark-gradient)" />
      <g
        stroke="oklch(0.985 0.005 245)"
        strokeWidth="2"
        strokeLinecap="round"
        fill="none"
        opacity="0.9"
      >
        <path d="M13 20h14" />
        <path d="M13 20l6.5-6.5" />
        <path d="M13 20l6.5 6.5" />
        <path d="M27 20l-6.5-6.5" />
        <path d="M27 20l-6.5 6.5" />
      </g>
      <circle cx="13" cy="20" r="2.8" fill="oklch(0.985 0.005 245)" />
      <circle cx="27" cy="20" r="2.8" fill="oklch(0.985 0.005 245)" />
      <circle cx="19.5" cy="13.5" r="2.2" fill="oklch(0.985 0.005 245)" opacity="0.85" />
      <circle cx="19.5" cy="26.5" r="2.2" fill="oklch(0.985 0.005 245)" opacity="0.85" />
    </svg>
  );
}
