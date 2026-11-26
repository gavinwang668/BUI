"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { Menu, Search, X } from "lucide-react"
import { href } from "@/lib/href"

/** One sidebar link. `hint` is what the search shows beside it (the group's label when unset). */
export type ShellLink = { href: string; label: string; hint?: string }

/**
 * A sidebar group. `filter`: the sidebar filter narrows it, with its count beside the label; the
 * other groups (a guide) step aside while the filter is in use.
 */
export type ShellGroup = { label: string; links: ShellLink[]; filter?: boolean }

/** What an addon (/charts, /panels) puts in the shell: its name, home, sidebar and search words. */
export type ShellProps = {
  name: string
  home: string
  groups: ShellGroup[]
  /** Placeholder of the sidebar filter, e.g. "Filter charts". */
  filterHint: string
  /** Placeholder of the search, e.g. "Search charts, families and guides". */
  searchHint: string
  /** Shown when the search finds nothing. */
  emptyHint: string
}

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

function Palette({ name, groups, searchHint, emptyHint, onClose }: Pick<ShellProps, "name" | "groups" | "searchHint" | "emptyHint"> & { onClose: () => void }) {
  const [query, setQuery] = useState("")
  const [cursor, setCursor] = useState(0)
  const input = useRef<HTMLInputElement>(null)
  const pages = useMemo(() => groups.flatMap((group) => group.links.map((link) => ({ ...link, hint: link.hint ?? group.label }))), [groups])
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
        aria-label={`Search ${name}`}
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
            placeholder={searchHint}
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
          {found.length === 0 && <li className="px-3 py-6 text-center text-sm text-(--muted)">{emptyHint}</li>}
        </ul>
      </div>
    </div>
  )
}

/** An addon's site (/charts, /panels): its bar under the site header, a sidebar you filter, a search, the page. */
export function AddonShell({ name, home, groups, filterHint, searchHint, emptyHint, pathname: path, children }: ShellProps & { pathname: string; children: React.ReactNode }) {
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
    // the site header's search button searches this addon here
    const click = (event: MouseEvent) => {
      if ((event.target as Element | null)?.closest("[data-search-open]")) setSearching(true)
    }
    window.addEventListener("keydown", handler)
    document.addEventListener("click", click)
    return () => {
      window.removeEventListener("keydown", handler)
      document.removeEventListener("click", click)
    }
  }, [])

  const needle = filter.trim().toLowerCase()
  const match = (name: string) => !needle || name.toLowerCase().includes(needle)
  const nav = (
    <nav aria-label={`${name} documentation`} className="flex flex-col px-3 pb-10">
      <label className="sticky top-0 z-10 -mx-3 mb-1 block bg-background px-3 pb-2 pt-3">
        <span className="sr-only">Filter the sidebar</span>
        <input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={filterHint}
          className="h-10 w-full border border-border bg-transparent px-3 text-sm outline-none transition-[border-color,box-shadow] duration-150 ease-out placeholder:text-(--muted) focus:border-primary focus:shadow-[0_0_0_3px_color-mix(in_oklab,var(--primary)_22%,transparent)]"
        />
      </label>
      {groups.map((group) => {
        if (!group.filter && needle) return null
        const shown = group.filter ? group.links.filter((link) => match(link.label)) : group.links
        return (
          <Group key={group.label} label={group.label} count={group.filter ? shown.length : undefined}>
            {shown.map((link) => (
              <NavLink key={link.href} href={link.href} active={pathname === link.href.replace(/\/$/, "")}>
                {link.label}
              </NavLink>
            ))}
          </Group>
        )
      })}
    </nav>
  )

  return (
    <div className="mx-auto flex min-h-[calc(100dvh-var(--header-h))] w-[94%] flex-col border-x border-border font-sans md:w-[86%]">
      <div className="sticky top-(--header-h) z-20 flex h-12 shrink-0 items-center gap-1 border-b border-border bg-background/90 px-3 backdrop-blur supports-backdrop-filter:bg-background/75 md:px-4">
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-label={open ? `Close the ${name.toLowerCase()} list` : `Open the ${name.toLowerCase()} list`}
          aria-expanded={open}
          className="inline-flex size-10 items-center justify-center transition-[background-color,scale] duration-150 ease-out hover:bg-foreground/8 active:scale-[0.96] md:hidden"
        >
          {open ? <X size={20} strokeWidth={1.5} /> : <Menu size={20} strokeWidth={1.5} />}
        </button>
        <nav aria-label="Breadcrumb" className="flex items-center gap-1 text-sm">
          <a href={href("addons")} className="flex h-10 items-center px-1.5 text-(--muted) transition-colors duration-150 ease-out hover:text-foreground">
            Addons
          </a>
          <span className="text-(--muted)" aria-hidden="true">
            /
          </span>
          <a href={home} className="flex h-10 items-center px-1.5 font-medium uppercase tracking-[0.14em]">
            {name}
          </a>
        </nav>
      </div>

      <div className="flex-1 md:grid md:grid-cols-[272px_minmax(0,1fr)]">
        <aside className="hidden border-r border-border md:sticky md:top-[calc(var(--header-h)+3rem)] md:block md:h-[calc(100dvh-var(--header-h)-3rem)] md:overflow-y-auto">{nav}</aside>
        {open && <aside className="fixed inset-x-0 bottom-0 top-[calc(var(--header-h)+3rem)] z-20 overflow-y-auto bg-background md:hidden">{nav}</aside>}
        <div className="min-w-0">{children}</div>
      </div>
      {searching && <Palette name={name.toLowerCase()} groups={groups} searchHint={searchHint} emptyHint={emptyHint} onClose={() => setSearching(false)} />}
    </div>
  )
}
