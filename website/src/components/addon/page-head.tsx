import { TriangleLine } from "./zigzag"

/** The top of an addon page (/charts, /panels): a kicker, the title in light capitals, what follows it, the zigzag. */
export function PageHead({ kicker, muted = false, title, children }: { kicker: React.ReactNode; muted?: boolean; title: React.ReactNode; children?: React.ReactNode }) {
  return (
    <header className="border-b border-border px-4 pb-8 pt-8 sm:px-8 md:pt-12">
      <p className={`mb-4 text-xs font-semibold uppercase tracking-[0.16em] ${muted ? "text-(--muted)" : "text-primary"}`}>{kicker}</p>
      <h1 className="text-4xl font-light uppercase tracking-tight text-balance md:text-6xl">{title}</h1>
      {children}
      <TriangleLine className="mt-8 w-full max-w-md" height={14} peakWidth={20} strokeWidth={2} />
    </header>
  )
}
