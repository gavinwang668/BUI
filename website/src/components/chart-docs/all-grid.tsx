"use client"

import { parseAsString, parseAsStringLiteral, useQueryStates } from "nuqs"
import { Search } from "lucide-react"
import { CardGrid, ChartCard, type CardItem } from "./chart-card"

type Item = CardItem & { group: "base" | "derived" }

const params = {
  q: parseAsString.withDefault(""),
  g: parseAsStringLiteral(["all", "base", "derived"] as const).withDefault("all"),
  f: parseAsString.withDefault(""),
}

const chip =
  "inline-flex h-10 items-center rounded-full border border-border px-3.5 text-sm transition-[background-color,color,border-color,scale] duration-150 ease-out hover:bg-foreground/6 active:scale-[0.96] aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-white"

export function AllGrid({ items, families }: { items: Item[]; families: string[] }) {
  const [{ q, g, f }, setParams] = useQueryStates(params, { shallow: true, history: "replace" })
  const needle = q.trim().toLowerCase()
  const shown = items.filter(
    (item) =>
      (g === "all" || item.group === g) &&
      (!f || item.family === f) &&
      (!needle || `${item.name} ${item.tag} ${item.family} ${item.tagline ?? ""}`.toLowerCase().includes(needle)),
  )

  return (
    <div>
      <div className="flex flex-col gap-4 border-b border-border p-4 sm:px-8">
        <label className="relative block">
          <span className="sr-only">Search the charts</span>
          <Search size={18} strokeWidth={1.5} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-(--muted)" />
          <input
            value={q}
            onChange={(event) => setParams({ q: event.target.value || null })}
            placeholder="Search by name, family or what it does"
            className="h-11 w-full border border-border bg-transparent pl-10 pr-3 text-sm outline-none transition-[border-color,box-shadow] duration-150 ease-out placeholder:text-(--muted) focus:border-primary focus:shadow-[0_0_0_3px_color-mix(in_oklab,var(--primary)_22%,transparent)]"
          />
        </label>
        <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Filter by kind">
          {(["all", "base", "derived"] as const).map((value) => (
            <button key={value} type="button" className={chip} aria-pressed={g === value} onClick={() => setParams({ g: value === "all" ? null : value })}>
              {value === "all" ? "Everything" : value === "base" ? "Base" : "Derived"}
            </button>
          ))}
          <span className="mx-1 hidden h-5 w-px bg-border sm:block" />
          {families.map((family) => (
            <button key={family} type="button" className={chip} aria-pressed={f === family} onClick={() => setParams({ f: f === family ? null : family })}>
              {family}
            </button>
          ))}
        </div>
        <p className="tabular text-sm text-(--muted)" aria-live="polite">
          {shown.length} of {items.length} charts
        </p>
      </div>
      {shown.length ? (
        <CardGrid>
          {shown.map((item, index) => (
            <ChartCard key={item.slug} item={item} priority={index < 4} />
          ))}
        </CardGrid>
      ) : (
        <p className="p-12 text-center text-(--muted)">No chart matches that. Charts are many, typos are fewer. Try clearing a filter.</p>
      )}
    </div>
  )
}
