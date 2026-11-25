import { TriangleLine } from "./zigzag"
import { Hatch, Section } from "./hatch"
import { Bullets, Prose, Rich, Steps, Tex } from "./rich"
import { bySlug, charts, imageOf, neighbours, sections, type ChartMeta } from "@/data/charts/registry"
import { loadDoc, loadGenerated, type Generated } from "@/data/charts/docs"
import type { ChartDoc } from "@/data/charts/types"
import { href } from "@/lib/href"

const card = "group flex flex-col gap-1 bg-background p-5 transition-[background-color,color] duration-150 ease-out hover:bg-primary hover:text-white focus-visible:bg-primary focus-visible:text-white"

export function staticPaths() {
  return charts.map((chart) => ({ params: { slug: chart.slug } }))
}

/** What a chart's page shows, and its head: the Astro page loads it and passes it to <ChartPage>. */
export async function loadChart(slug: string) {
  const meta = bySlug.get(slug)
  if (!meta) throw new Error(`no chart ${slug}`)
  const [doc, generated] = await Promise.all([loadDoc(slug), loadGenerated(slug)])
  return { meta, doc, generated, title: meta.name, description: doc?.tagline ?? meta.blurb, image: imageOf(slug) }
}

/** The tabs of the page's table of contents, for the <Toc> island. */
export function tocItems(doc: ChartDoc | null) {
  const hasMedia = (doc?.media.length ?? 0) > 0
  const written = ["overview", "math", "when", "read", "story", "pitfalls", "related"]
  return sections
    .filter((section) => {
      if (section.id === "watch") return hasMedia
      if (written.includes(section.id)) return Boolean(doc)
      return true
    })
    .map(({ id, label }) => ({ id, label }))
}

// Astro passes these as named slots, which TypeScript does not see as props
type Islands = { stage?: React.ReactNode; examples?: React.ReactNode; media?: React.ReactNode; toc?: React.ReactNode }

/** The islands (the live stage, the example list, the videos and the table of contents) come from the Astro page. */
export default function ChartPage({ meta, doc, generated, stage, examples, media, toc }: { meta: ChartMeta; doc: ChartDoc | null; generated: Generated } & Islands) {
  const slug = meta.slug
  const { previous, next } = neighbours(slug)
  const hasMedia = (doc?.media.length ?? 0) > 0

  return (
    <article className="charts-doc">
      <header className="border-b border-border px-4 pb-8 pt-8 sm:px-8 md:pt-12">
        <p className="mb-4 text-xs font-semibold uppercase tracking-[0.16em] text-(--muted)">
          <a href={href("charts/all")} className="hover:text-foreground">
            Charts
          </a>
          <span className="mx-2">/</span>
          {meta.group === "base" ? "Base" : meta.family}
        </p>
        <h1 className="text-4xl font-light uppercase tracking-tight text-balance md:text-6xl">{meta.name}</h1>
        <p className="mt-3 font-mono text-sm text-primary">
          &lt;{meta.tag} /&gt; <span className="text-(--muted)">· {meta.examples} examples</span>
        </p>
        {doc && <p className="mt-5 max-w-2xl text-lg text-pretty md:text-xl">{doc.tagline}</p>}
        <TriangleLine className="mt-8 w-full max-w-md" height={14} peakWidth={20} strokeWidth={2} />
      </header>

      <div className="xl:grid xl:grid-cols-[minmax(0,1fr)_220px]">
        <div className="min-w-0 xl:border-r xl:border-border">
          {stage}

          {doc ? (
            <Section id="overview" title="Overview">
              <div className="max-w-2xl text-base leading-7 md:text-[17px] md:leading-8">
                <Prose paragraphs={doc.intro} />
              </div>
            </Section>
          ) : (
            <Section id="overview" title="Write-up in progress">
              <p className="max-w-xl text-(--muted)">The explanation for this chart is still being written. The live render, examples and API below already work.</p>
            </Section>
          )}

          <Section id="api" title="API" kicker="Props">
            <div className="api-doc max-w-3xl" dangerouslySetInnerHTML={{ __html: generated.api }} />
          </Section>

          <Section id="examples" title={`${generated.examples.length} examples`} kicker="Run them">
            <p className="mb-6 max-w-2xl text-(--muted)">Press Run to load an example into the live render at the top of the page. Open one to read its code.</p>
            <div className="-mx-4 overflow-hidden border-y border-border sm:-mx-8">
              {examples}
            </div>
          </Section>

          {doc && (
            <>
              <Hatch className="h-8" />

              <Section id="read" title="How to read it">
                <div className="max-w-2xl text-base leading-7 md:text-[17px] md:leading-8">
                  <Steps items={doc.read} />
                </div>
              </Section>

              <Section id="when" title="When to use it, and when not to">
                <div className="grid gap-px overflow-hidden border border-border bg-border md:grid-cols-2">
                  <div className="bg-background p-5">
                    <h3 className="mb-4 text-sm font-semibold uppercase tracking-[0.12em] text-primary">Reach for it when</h3>
                    <Bullets items={doc.use} />
                  </div>
                  <div className="bg-background p-5">
                    <h3 className="mb-4 text-sm font-semibold uppercase tracking-[0.12em] text-(--muted)">Skip it when</h3>
                    <Bullets items={doc.avoid} />
                  </div>
                </div>
              </Section>

              <Section id="math" title="The math behind it">
                <div className="flex max-w-2xl flex-col gap-10 text-base leading-7 md:text-[17px] md:leading-8">
                  {doc.math.map((block) => (
                    <div key={block.heading} className="flex flex-col gap-3">
                      <h3 className="text-lg font-semibold leading-snug">{block.heading}</h3>
                      <Prose paragraphs={block.body} />
                      {block.tex?.map((line) => <Tex key={line} source={line} />)}
                    </div>
                  ))}
                </div>
              </Section>

              <Hatch className="h-8" />

              <Section id="story" title={doc.story.title} kicker="Real life usage">
                <div className="max-w-2xl text-base leading-7 md:text-[17px] md:leading-8">
                  <Prose paragraphs={doc.story.paragraphs} />
                </div>
              </Section>

              <Section id="pitfalls" title="Pitfalls">
                <div className="max-w-2xl text-base leading-7 md:text-[17px] md:leading-8">
                  <Bullets items={doc.pitfalls} />
                </div>
              </Section>
            </>
          )}

          {doc && hasMedia && (
            <Section id="watch" title="Watch and read" kicker="From around the web">
              {media}
            </Section>
          )}

          {doc && (
            <Section id="related" title="Related charts" kicker="Keep going">
              <div className="grid gap-px overflow-hidden border border-border bg-border md:grid-cols-2">
                {doc.related.map((related) => {
                  const target = bySlug.get(related.slug)
                  if (!target) return null
                  return (
                    <a key={related.slug} href={href(`charts/${related.slug}`)} className={card}>
                      <span className="text-sm font-semibold uppercase tracking-[0.08em]">{target.name}</span>
                      <span className="text-sm text-(--muted) transition-[color] duration-150 ease-out group-hover:text-white/90 group-focus-visible:text-white/90">
                        <Rich text={related.why} />
                      </span>
                    </a>
                  )
                })}
              </div>
            </Section>
          )}

          <nav aria-label="Previous and next chart" className="grid grid-cols-2 gap-px border-b border-border bg-border">
            {previous ? (
              <a href={href(`charts/${previous.slug}`)} className={`${card} min-h-20 justify-center`}>
                <span className="text-xs uppercase tracking-[0.14em] text-(--muted) group-hover:text-white/80">Previous</span>
                <span className="font-medium">{previous.name}</span>
              </a>
            ) : (
              <span className="bg-background" />
            )}
            {next ? (
              <a href={href(`charts/${next.slug}`)} className={`${card} min-h-20 items-end justify-center text-right`}>
                <span className="text-xs uppercase tracking-[0.14em] text-(--muted) group-hover:text-white/80">Next</span>
                <span className="font-medium">{next.name}</span>
              </a>
            ) : (
              <span className="bg-background" />
            )}
          </nav>
          <div aria-hidden="true" className="hatch h-[45vh] min-h-48" />
        </div>
        {toc}
      </div>
    </article>
  )
}
