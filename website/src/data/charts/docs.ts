import { loaders } from "./loaders"
import type { ChartDoc } from "./types"
import { href } from "../../lib/href"

export type Generated = {
  api: string
  examples: { id: string; title: string; description: string; code: string }[]
}

export async function loadDoc(slug: string): Promise<ChartDoc | null> {
  const load = loaders[slug]
  return load ? (await load()).doc : null
}

export async function loadGenerated(slug: string): Promise<Generated> {
  const generated = (await import(`./generated/${slug}.json`)).default as Generated
  // the generator writes site-absolute /charts links: put them under the deploy's base
  return { ...generated, api: generated.api.replaceAll('href="/charts/', `href="${href("charts/")}`) }
}
