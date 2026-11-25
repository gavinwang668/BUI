import { imageOf } from "@/data/charts/registry"
import { href } from "@/lib/href"

export type CardItem = {
  slug: string
  name: string
  tag: string
  family: string
  tagline?: string
  note?: string
}

const quick = [
  { id: "math", label: "Math" },
  { id: "examples", label: "Examples" },
  { id: "api", label: "API" },
]

export function ChartCard({ item, priority = false }: { item: CardItem; priority?: boolean }) {
  const blurb = item.note ?? item.tagline
  return (
    <article className="group relative flex flex-col gap-3 bg-background p-4">
      <a href={href(`charts/${item.slug}`)} className="flex flex-col gap-3 after:absolute after:inset-0 after:content-[''] focus-visible:outline-2 focus-visible:outline-primary">
        <div className="img-outline overflow-hidden bg-(--surface)">
          <img
            src={imageOf(item.slug)}
            alt={`${item.name} preview`}
            width={480}
            height={270}
            fetchPriority={priority ? "high" : undefined}
            loading={priority ? undefined : "lazy"}
            className="aspect-video w-full object-cover transition-[scale] duration-300 ease-out group-hover:scale-[1.03]"
          />
        </div>
        <div className="flex flex-col gap-0.5">
          <h3 className="text-sm font-semibold uppercase tracking-[0.08em]">{item.name}</h3>
          <p className="font-mono text-xs text-primary">&lt;{item.tag}&gt;</p>
        </div>
      </a>
      {blurb && <p className="text-sm text-(--muted) text-pretty">{blurb}</p>}
      <div className="relative z-10 mt-auto flex flex-wrap gap-x-1 pt-1 text-xs">
        {quick.map((link) => (
          <a
            key={link.id}
            href={href(`charts/${item.slug}#${link.id}`)}
            className="inline-flex min-h-8 items-center px-2 text-(--muted) transition-[background-color,color] duration-150 ease-out hover:bg-foreground/8 hover:text-foreground"
          >
            {link.label}
          </a>
        ))}
      </div>
    </article>
  )
}

export function CardGrid({ children }: { children: React.ReactNode }) {
  return <div className="grid gap-px border-b border-border bg-border sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">{children}</div>
}
