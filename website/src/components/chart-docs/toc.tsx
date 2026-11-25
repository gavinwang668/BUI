"use client"

import { useEffect, useState } from "react"

export function Toc({ items }: { items: { id: string; label: string }[] }) {
  const [active, setActive] = useState(items[0]?.id ?? "")

  useEffect(() => {
    const targets = items.map((item) => document.getElementById(item.id)).filter((element): element is HTMLElement => Boolean(element))
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((entry) => entry.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)
        if (visible[0]) setActive(visible[0].target.id)
      },
      { rootMargin: "-72px 0px -65% 0px" },
    )
    targets.forEach((target) => observer.observe(target))
    return () => observer.disconnect()
  }, [items])

  return (
    <nav aria-label="On this page" className="sticky top-14 hidden max-h-[calc(100dvh-3.5rem)] overflow-y-auto p-5 xl:block">
      <p className="mb-3 text-[11px] font-semibold uppercase tracking-[0.14em] text-(--muted)">On this page</p>
      <ul className="flex flex-col border-l border-border">
        {items.map((item) => (
          <li key={item.id}>
            <a
              href={`#${item.id}`}
              onClick={(event) => {
                const target = document.getElementById(item.id)
                if (!target) return
                event.preventDefault()
                const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
                target.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" })
                window.history.replaceState(window.history.state, "", `#${item.id}`)
                setActive(item.id)
              }}
              aria-current={active === item.id ? "true" : undefined}
              className="-ml-px flex min-h-10 items-center border-l border-transparent pl-3 text-sm text-(--muted) transition-[color,border-color] duration-150 ease-out hover:text-foreground aria-[current=true]:border-primary aria-[current=true]:font-medium aria-[current=true]:text-foreground"
            >
              {item.label}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  )
}
