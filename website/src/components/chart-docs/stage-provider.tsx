"use client"

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react"
import { href } from "@/lib/href"
// the one page-side host for blenderui apps (addons/bui-charts/web is one)
import { mount, putString } from "../../../../bindings/rust/blenderui-web/js/bui-web.js"

type Engine = {
  bui_charts_show: (name: number, length: number, example: number) => void
  bui_charts_set_dark: (dark: number) => void
  bui_charts_reseed: (seed: number) => void
}

export type StageApi = {
  show: (name: string, example: number) => void
  setDark: (dark: boolean) => void
  reseed: (seed: number) => void
}

type Status = "idle" | "loading" | "ready" | "error"

type StageContext = {
  status: Status
  api: StageApi | null
  attach: (slot: HTMLElement | null) => void
}

const Context = createContext<StageContext | null>(null)

const BINARY = href("charts/wasm/bui-charts.wasm")

type Booted = { canvas: HTMLCanvasElement; api: StageApi }

let booting: Promise<Booted> | null = null
let booted: Booted | null = null
let parking: HTMLDivElement | null = null
let slot: HTMLElement | null = null

const park = () => {
  if (parking) return parking
  parking = document.createElement("div")
  parking.setAttribute("aria-hidden", "true")
  parking.style.cssText = "position:fixed;left:-10000px;top:0;width:960px;height:540px;pointer-events:none;overflow:hidden"
  document.body.appendChild(parking)
  return parking
}

const place = () => {
  if (!booted) return
  const target = slot ?? park()
  if (booted.canvas.parentElement !== target) target.appendChild(booted.canvas)
}

const start = () => {
  booting ??= (async () => {
    const canvas = document.createElement("canvas")
    canvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;outline:none;touch-action:none"
    park().appendChild(canvas)
    const { exports } = await mount(canvas, BINARY)
    const engine = exports as Engine
    booted = {
      canvas,
      api: {
        show: (name, example) => {
          const [text, length] = putString(exports, name) as [number, number]
          engine.bui_charts_show(text, length, example)
        },
        setDark: (dark) => engine.bui_charts_set_dark(dark ? 1 : 0),
        reseed: (seed) => engine.bui_charts_reseed(seed),
      },
    }
    place()
    return booted
  })()
  return booting
}

export function useStage() {
  const stage = useContext(Context)
  if (!stage) throw new Error("useStage needs a StageProvider")
  return stage
}

export function StageProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<Status>(booted ? "ready" : "idle")
  const [api, setApi] = useState<StageApi | null>(booted?.api ?? null)

  useEffect(() => {
    if (booted) {
      setApi(booted.api)
      setStatus("ready")
    }
  }, [])

  const attach = useCallback((element: HTMLElement | null) => {
    slot = element
    place()
    if (!element || booted) return
    setStatus("loading")
    start().then(
      (ready) => {
        setApi(ready.api)
        setStatus("ready")
      },
      (error) => {
        console.error("bui-charts engine failed", error)
        booting = null
        setStatus("error")
      },
    )
  }, [])

  const value = useMemo(() => ({ status, api, attach }), [status, api, attach])
  return <Context value={value}>{children}</Context>
}
