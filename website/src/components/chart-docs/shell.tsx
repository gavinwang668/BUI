"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { Menu, Search, X } from "lucide-react"
import { baseCharts, charts, derivedCharts } from "@/data/charts/registry"
import { href } from "@/lib/href"

const guide = [
  { href: href("charts"), label: "Getting started" },
  { href: href("charts/all"), label: "All charts" },
  { href: href("charts/which"), label: "Which chart to use?" },
  { href: href("charts/domains"), label: "Domain specific" },
]

function NavLink({ href, active, children }: { href: string; active: boolean; children: React.ReactNode }) {
  return (
    <a
      href={href}
      aria-current={active ? "page" : undefined}
      className="flex min-h-10 items-center px-3 text-sm text-(--muted) transition-[background-color,color] duration-150 ease-out hover:bg-foreground/6 hover:text-foreground aria-[current=page]:bg-primary aria-[current=page]:font-medium aria-[current=page]:text-white"
    >
      {children}
    </a>
  )
}

function Group({ label, count, children }: { label: string; count?: number; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex items-baseline justify-between px-3 pb-1 pt-4 text-[11px] font-semibold uppercase tracking-[0.12em] text-(--muted)">
        <span>{label}</span>
        {count !== undefined && <span className="tabular font-normal">{count}</span>}
      </div>
      {children}
    </div>
  )
}

function Palette({ onClose }: { onClose: () => void }) {
  const [query, setQuery] = useState("")
  const [cursor, setCursor] = useState(0)
  const input = useRef<HTMLInputElement>(null)
  const pages = useMemo(
    () => [
      ...guide.map((page) => ({ href: page.href, label: page.label, hint: "Guide" })),
      ...charts.map((chart) => ({ href: href(`charts/${chart.slug}`), label: chart.name, hint: chart.family })),
    ],
    [],
  )
  const found = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return pages.slice(0, 12)
    return pages.filter((page) => `${page.label} ${page.hint}`.toLowerCase().includes(needle)).slice(0, 12)
  }, [pages, query])

  useEffect(() => input.current?.focus(), [])

  const go = (href: string) => {
    onClose()
    window.location.assign(href)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 px-4 pt-[12vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-label="Search the charts"
        className="w-full max-w-xl overflow-hidden border border-border bg-background shadow-[0_24px_60px_-20px_rgb(0_0_0/0.5)]"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border px-4">
          <Search size={18} strokeWidth={1.5} />
          <input
            ref={input}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value)
              setCursor(0)
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault()
                setCursor((value) => Math.min(value + 1, found.length - 1))
              } else if (event.key === "ArrowUp") {
                event.preventDefault()
                setCursor((value) => Math.max(value - 1, 0))
              } else if (event.key === "Enter" && found[cursor]) go(found[cursor].href)
              else if (event.key === "Escape") onClose()
            }}
            placeholder="Search charts, families and guides"
            className="h-12 flex-1 bg-transparent text-sm outline-none placeholder:text-(--muted)"
          />
        </div>
        <ul className="max-h-[50vh] overflow-y-auto p-2">
          {found.map((page, index) => (
            <li key={page.href}>
              <button
                type="button"
                onMouseMove={() => setCursor(index)}
                onClick={() => go(page.href)}
                aria-selected={index === cursor}
                className="flex min-h-10 w-full items-center justify-between gap-4 px-3 text-left text-sm aria-selected:bg-primary/12"
              >
                <span>{page.label}</span>
                <span className="truncate text-xs text-(--muted)">{page.hint}</span>
              </button>
            </li>
          ))}
          {found.length === 0 && <li className="px-3 py-6 text-center text-sm text-(--muted)">Nothing matches that. Try a family name, like Finance.</li>}
        </ul>
      </div>
    </div>
  )
}

export function ChartsShell({ pathname: path, actions, children }: { pathname: string; actions?: React.ReactNode; children: React.ReactNode }) {
  const pathname = path.replace(/\/$/, "") || "/"
  const [open, setOpen] = useState(false)
  const [searching, setSearching] = useState(false)
  const [filter, setFilter] = useState("")

  useEffect(() => setOpen(false), [pathname])

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        setSearching((value) => !value)
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [])

  const needle = filter.trim().toLowerCase()
  const match = (name: string) => !needle || name.toLowerCase().includes(needle)
  const shownBase = baseCharts.filter((chart) => match(chart.name))
  const shownDerived = derivedCharts.filter((chart) => match(chart.name))
  const nav = (
    <nav aria-label="Charts documentation" className="flex flex-col px-3 pb-10">
      <label className="sticky top-0 z-10 -mx-3 mb-1 block bg-background px-3 pb-2 pt-3">
        <span className="sr-only">Filter the sidebar</span>
        <input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter charts"
          className="h-10 w-full border border-border bg-transparent px-3 text-sm outline-none transition-[border-color,box-shadow] duration-150 ease-out placeholder:text-(--muted) focus:border-primary focus:shadow-[0_0_0_3px_color-mix(in_oklab,var(--primary)_22%,transparent)]"
        />
      </label>
      {!needle && (
        <Group label="Guide">
          {guide.map((page) => (
            <NavLink key={page.href} href={page.href} active={pathname === page.href}>
              {page.label}
            </NavLink>
          ))}
        </Group>
      )}
      <Group label="Base charts" count={shownBase.length}>
        {shownBase.map((chart) => (
          <NavLink key={chart.slug} href={href(`charts/${chart.slug}`)} active={pathname === href(`charts/${chart.slug}`)}>
            {chart.name}
          </NavLink>
        ))}
      </Group>
      <Group label="Derived charts" count={shownDerived.length}>
        {shownDerived.map((chart) => (
          <NavLink key={chart.slug} href={href(`charts/${chart.slug}`)} active={pathname === href(`charts/${chart.slug}`)}>
            {chart.name}
          </NavLink>
        ))}
      </Group>
    </nav>
  )

  return (
    <div className="mx-auto flex min-h-dvh w-[94%] flex-col border-x border-border font-sans md:w-[86%]">
      <header className="sticky top-0 z-30 flex h-14 shrink-0 items-center gap-2 border-b border-border bg-background/90 pl-3 pr-4 backdrop-blur md:pr-10 supports-backdrop-filter:bg-background/75">
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-label={open ? "Close the menu" : "Open the menu"}
          aria-expanded={open}
          className="inline-flex size-10 items-center justify-center transition-[background-color,scale] duration-150 ease-out hover:bg-foreground/8 active:scale-[0.96] md:hidden"
        >
          {open ? <X size={20} strokeWidth={1.5} /> : <Menu size={20} strokeWidth={1.5} />}
        </button>
        <a href={href("")} className="flex h-10 items-center gap-2.5 px-2 transition-[background-color] duration-150 ease-out hover:bg-foreground/6">
          <img src={href("logo.png")} alt="BUI" width={27} height={22} className="h-[22px] w-auto" />
        </a>
        <span className="text-(--muted)" aria-hidden="true">
          /
        </span>
        <a href={href("charts")} className="flex h-10 items-center px-1.5 text-sm font-medium uppercase tracking-[0.14em]">
          Charts
        </a>
        <button
          type="button"
          onClick={() => setSearching(true)}
          className="ml-auto inline-flex h-10 items-center gap-2 border border-border px-3 text-sm text-(--muted) transition-[background-color,color,scale] duration-150 ease-out hover:bg-foreground/6 hover:text-foreground active:scale-[0.96]"
        >
          <Search size={16} strokeWidth={1.5} />
          <span className="hidden sm:inline">Search</span>
          <kbd className="hidden border border-border px-1.5 text-[11px] sm:inline">Ctrl K</kbd>
        </button>
        {actions}
      </header>

      <div className="flex-1 md:grid md:grid-cols-[272px_minmax(0,1fr)]">
        <aside className="hidden border-r border-border md:sticky md:top-14 md:block md:h-[calc(100dvh-3.5rem)] md:overflow-y-auto">{nav}</aside>
        {open && <aside className="fixed inset-x-0 bottom-0 top-14 z-20 overflow-y-auto bg-background md:hidden">{nav}</aside>}
        <div className="min-w-0">{children}</div>
      </div>
      {searching && <Palette onClose={() => setSearching(false)} />}
    </div>
  )
}
