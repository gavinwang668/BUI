/** A link card of the addon pages: fills with the selection blue on hover and focus. */
export const linkCard =
  "group flex flex-col gap-1 bg-background p-5 transition-[background-color,color] duration-150 ease-out hover:bg-primary hover:text-white focus-visible:bg-primary focus-visible:text-white"

type Page = { href: string; label: string }

/** The previous and next pages at the foot of an addon page. */
export function PrevNext({ label, previous, next }: { label: string; previous?: Page; next?: Page }) {
  return (
    <nav aria-label={label} className="grid grid-cols-2 gap-px border-b border-border bg-border">
      {previous ? (
        <a href={previous.href} className={`${linkCard} min-h-20 justify-center`}>
          <span className="text-xs uppercase tracking-[0.14em] text-(--muted) group-hover:text-white/80">Previous</span>
          <span className="font-medium">{previous.label}</span>
        </a>
      ) : (
        <span className="bg-background" />
      )}
      {next ? (
        <a href={next.href} className={`${linkCard} min-h-20 items-end justify-center text-right`}>
          <span className="text-xs uppercase tracking-[0.14em] text-(--muted) group-hover:text-white/80">Next</span>
          <span className="font-medium">{next.label}</span>
        </a>
      ) : (
        <span className="bg-background" />
      )}
    </nav>
  )
}
