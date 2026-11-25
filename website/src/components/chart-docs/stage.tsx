"use client"

import { useEffect, useRef, useState } from "react"
import { parseAsInteger, parseAsStringLiteral, useQueryStates } from "nuqs"
import { Check, ChevronLeft, ChevronRight, Link2, Moon, Shuffle, Sun } from "lucide-react"
import { useStage } from "./stage-provider"

export const stageParams = {
  ex: parseAsInteger.withDefault(0),
  seed: parseAsInteger.withDefault(0),
  theme: parseAsStringLiteral(["light", "dark"] as const),
}

type Item = { title: string; description: string }

const button =
  "inline-flex h-10 min-w-10 items-center justify-center gap-1.5 px-2.5 text-sm font-medium text-foreground transition-[background-color,scale] duration-150 ease-out hover:bg-foreground/8 active:scale-[0.96] disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-primary"

// the site's theme: <html data-theme>, set by Base.astro and the theme toggle
function useSystemDark() {
  const [dark, setDark] = useState(false)
  useEffect(() => {
    const root = document.documentElement
    const update = () => setDark(root.dataset.theme === "dark")
    update()
    const observer = new MutationObserver(update)
    observer.observe(root, { attributes: true, attributeFilter: ["data-theme"] })
    return () => observer.disconnect()
  }, [])
  return dark
}

export function Stage({ name, image, items }: { name: string; image: string; items: Item[] }) {
  const { status, api, attach } = useStage()
  const [{ ex, seed, theme }, setParams] = useQueryStates(stageParams, { shallow: true, history: "replace" })
  const system = useSystemDark()
  const dark = theme ? theme === "dark" : system
  const index = Math.min(Math.max(ex, 0), items.length - 1)
  const slot = useRef<HTMLDivElement>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    attach(slot.current)
    return () => attach(null)
  }, [attach])

  useEffect(() => {
    if (!api) return
    api.setDark(dark)
  }, [api, dark])

  useEffect(() => {
    if (!api) return
    api.reseed(seed)
  }, [api, seed])

  useEffect(() => {
    if (!api) return
    api.show(name, index)
  }, [api, name, index, seed])

  const current = items[index]
  const step = (by: number) => setParams({ ex: (index + by + items.length) % items.length })
  const copy = async () => {
    await navigator.clipboard.writeText(window.location.href)
    setCopied(true)
    setTimeout(() => setCopied(false), 1600)
  }

  return (
    <section id="live" className="scroll-mt-20 border-b border-border">
      <div className="flex flex-wrap items-center gap-1 border-b border-border px-2 py-1.5 sm:px-3">
        <button type="button" className={button} onClick={() => step(-1)} aria-label="Previous example">
          <ChevronLeft size={18} strokeWidth={1.5} />
        </button>
        <span className="tabular min-w-14 text-center text-sm text-(--muted)">
          {index + 1} / {items.length}
        </span>
        <button type="button" className={button} onClick={() => step(1)} aria-label="Next example">
          <ChevronRight size={18} strokeWidth={1.5} />
        </button>
        <div className="mx-1 hidden h-5 w-px bg-border sm:block" />
        <button type="button" className={button} onClick={() => setParams({ seed: seed + 1 })} aria-label="Shuffle the data">
          <Shuffle size={18} strokeWidth={1.5} />
          <span className="hidden sm:inline">Shuffle data</span>
        </button>
        <button type="button" className={button} onClick={() => setParams({ theme: dark ? "light" : "dark" })} aria-label={dark ? "Use the light theme" : "Use the dark theme"}>
          {dark ? <Sun size={18} strokeWidth={1.5} /> : <Moon size={18} strokeWidth={1.5} />}
          <span className="hidden sm:inline">{dark ? "Light" : "Dark"}</span>
        </button>
        <button type="button" className={`${button} ml-auto`} onClick={copy} aria-label="Copy a link to this example">
          {copied ? <Check size={18} strokeWidth={1.5} /> : <Link2 size={18} strokeWidth={1.5} />}
          <span className="hidden md:inline">{copied ? "Copied" : "Share"}</span>
        </button>
      </div>

      <div className="relative bg-(--surface)" style={{ height: "clamp(340px, 62vh, 540px)" }}>
        <div ref={slot} className="absolute inset-0" />
        {status !== "ready" && (
          <div className="hatch absolute inset-0 flex items-center justify-center">
            {status === "error" ? (
              <div className="flex max-w-md flex-col items-center gap-3 bg-(--surface) p-5 text-center text-sm">
                <img src={image} alt={`${name} preview`} width={480} height={270} className="img-outline" />
                <p>This browser could not start the live engine, so here is a still of the chart instead.</p>
              </div>
            ) : (
              <p className="bg-(--surface) px-4 py-2 text-sm text-(--muted) outline outline-border">Starting the chart engine…</p>
            )}
          </div>
        )}
      </div>

      <div className="flex flex-col gap-1 border-t border-border px-4 py-3 sm:flex-row sm:items-baseline sm:gap-3">
        <h2 className="text-sm font-semibold uppercase tracking-wide">{current.title}</h2>
        <p className="text-sm text-(--muted)">{current.description}</p>
      </div>
    </section>
  )
}
