import registry from "./generated/registry.json"
import { href } from "../../lib/href"

export type ChartMeta = {
  slug: string
  name: string
  tag: string
  group: "base" | "derived"
  dir: string
  family: string
  blurb: string
  examples: number
}

export const charts = registry as ChartMeta[]

export const bySlug = new Map(charts.map((chart) => [chart.slug, chart]))

export const baseCharts = charts.filter((chart) => chart.group === "base")
export const derivedCharts = charts.filter((chart) => chart.group === "derived")

export const families = [...new Set(derivedCharts.map((chart) => chart.family))].map((family) => ({
  title: family,
  charts: derivedCharts.filter((chart) => chart.family === family),
}))

export const exampleCount = charts.reduce((total, chart) => total + chart.examples, 0)

export const imageOf = (slug: string) => href(`charts/images/${slug}.webp`)

export const neighbours = (slug: string) => {
  const index = charts.findIndex((chart) => chart.slug === slug)
  return { previous: charts[index - 1] ?? null, next: charts[index + 1] ?? null }
}

export const sections = [
  { id: "live", label: "Live" },
  { id: "overview", label: "Overview" },
  { id: "api", label: "API" },
  { id: "examples", label: "Examples" },
  { id: "read", label: "How to read it" },
  { id: "when", label: "When to use it" },
  { id: "math", label: "The math" },
  { id: "story", label: "Story" },
  { id: "pitfalls", label: "Pitfalls" },
  { id: "watch", label: "Watch and read" },
  { id: "related", label: "Related" },
] as const
