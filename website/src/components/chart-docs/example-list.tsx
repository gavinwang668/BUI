"use client"

import { useQueryStates } from "nuqs"
import { Play } from "lucide-react"
import { stageParams } from "./stage"

export type ExampleEntry = { id: string; title: string; description: string; code: string }

export function ExampleList({ items }: { items: ExampleEntry[] }) {
  const [{ ex }, setParams] = useQueryStates(stageParams, { shallow: true, history: "replace" })

  const run = (index: number) => {
    setParams({ ex: index })
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    document.getElementById("live")?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" })
  }

  return (
    <ol className="grid gap-px bg-border sm:grid-cols-1">
      {items.map((item, index) => (
        <li key={item.id} id={`example-${item.id}`} className="bg-background" aria-current={index === ex ? "true" : undefined}>
          <details className="group">
            <summary className="flex min-h-14 cursor-pointer list-none items-center gap-3 px-4 py-2 transition-[background-color] duration-150 ease-out hover:bg-foreground/4 group-aria-[current=true]:bg-primary/8 [&::-webkit-details-marker]:hidden">
              <span className="tabular w-7 shrink-0 text-sm font-semibold text-primary">{String(index + 1).padStart(2, "0")}</span>
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-sm font-medium">{item.title}</span>
                <span className="truncate text-sm text-(--muted)">{item.description}</span>
              </span>
              <button
                type="button"
                onClick={(event) => {
                  event.preventDefault()
                  run(index)
                }}
                className="inline-flex h-10 shrink-0 items-center gap-1.5 border border-border px-3 text-sm font-medium transition-[background-color,scale,color] duration-150 ease-out hover:bg-primary hover:text-white active:scale-[0.96]"
              >
                <Play size={14} strokeWidth={1.5} />
                Run
              </button>
            </summary>
            {item.code && <div className="code-block border-t border-border bg-(--code-bg)" dangerouslySetInnerHTML={{ __html: item.code }} />}
          </details>
        </li>
      ))}
    </ol>
  )
}
