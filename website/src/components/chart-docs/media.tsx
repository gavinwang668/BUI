"use client"

import { useState } from "react"
import { SiX, SiYoutube } from "@icons-pack/react-simple-icons"
import { ArrowUpRight, Play } from "lucide-react"
import type { Media } from "@/data/charts/types"

function Video({ id, title }: { id: string; title: string }) {
  const [playing, setPlaying] = useState(false)
  return (
    <div className="relative aspect-video overflow-hidden bg-black">
      {playing ? (
        <iframe
          src={`https://www.youtube-nocookie.com/embed/${id}?autoplay=1&rel=0`}
          title={title}
          allow="autoplay; encrypted-media; picture-in-picture"
          allowFullScreen
          className="absolute inset-0 size-full"
        />
      ) : (
        <button type="button" onClick={() => setPlaying(true)} aria-label={`Play ${title}`} className="group absolute inset-0 size-full">
          <img src={`https://i.ytimg.com/vi/${id}/hqdefault.jpg`} alt="" loading="lazy" className="size-full object-cover opacity-90 transition-[opacity,scale] duration-200 ease-out group-hover:opacity-100 group-hover:scale-[1.02]" />
          <span className="absolute left-1/2 top-1/2 flex size-14 -translate-x-1/2 -translate-y-1/2 items-center justify-center bg-primary text-white transition-[scale] duration-150 ease-out group-active:scale-[0.96]">
            <Play size={22} strokeWidth={1.5} />
          </span>
        </button>
      )}
    </div>
  )
}

export function MediaGrid({ items }: { items: Media[] }) {
  return (
    <div className="grid gap-6 md:grid-cols-2">
      {items.map((item) =>
        item.kind === "youtube" ? (
          <figure key={item.id} className="flex flex-col gap-3">
            <Video id={item.id} title={item.title} />
            <figcaption className="flex flex-col gap-1 text-sm">
              <a href={`https://www.youtube.com/watch?v=${item.id}`} target="_blank" rel="noopener noreferrer" className="font-medium underline-offset-4 hover:underline">
                <SiYoutube size={14} className="mr-2 inline-block align-[-2px] text-[#ff0033]" aria-hidden="true" />
                {item.title}
              </a>
              <span className="text-(--muted)">{item.channel}</span>
              <span>{item.note}</span>
            </figcaption>
          </figure>
        ) : (
          <figure key={item.url} className="flex flex-col gap-3">
            <blockquote className="flex flex-1 flex-col gap-3 border border-border bg-(--surface) p-5">
              <p className="text-[15px] leading-relaxed">{item.text}</p>
              <footer className="mt-auto flex items-center justify-between gap-3 text-sm text-(--muted)">
                <span className="inline-flex items-center gap-2">
                  <SiX size={13} aria-hidden="true" />
                  {item.author}
                </span>
                <a href={item.url} target="_blank" rel="noopener noreferrer" className="inline-flex h-10 items-center gap-1 px-2 font-medium text-foreground hover:bg-foreground/6">
                  View on X
                  <ArrowUpRight size={14} strokeWidth={1.5} />
                </a>
              </footer>
            </blockquote>
            <figcaption className="text-sm">{item.note}</figcaption>
          </figure>
        ),
      )}
    </div>
  )
}
