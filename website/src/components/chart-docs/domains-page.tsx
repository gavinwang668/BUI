import { PageHead } from "../addon/page-head"
import { CardGrid, ChartCard } from "./chart-card"
import { Hatch } from "./hatch"
import { bySlug } from "@/data/charts/registry"
import { domains } from "@/data/charts/domains"

export const metadata = {
  title: "Domain specific",
  description: "Charts grouped by the field you work in: operations, finance, manufacturing, electronics, biology, earth science and more.",
}

export default function DomainsPage() {
  return (
    <div className="charts-doc">
      <PageHead kicker="By field" title="Domain specific">
        <p className="mt-5 max-w-2xl text-lg text-pretty text-(--muted)">
          Every field has its own favourite charts, usually invented by someone who was tired of the previous one. Pick your field and start with the ones your colleagues will already recognise.
        </p>
      </PageHead>

      <nav aria-label="Domains" className="flex flex-wrap gap-2 border-b border-border p-4 sm:px-8">
        {domains.map((domain) => (
          <a
            key={domain.id}
            href={`#${domain.id}`}
            className="inline-flex min-h-10 items-center rounded-full border border-border px-4 text-sm transition-[background-color,color,scale] duration-150 ease-out hover:bg-primary hover:text-white active:scale-[0.96]"
          >
            {domain.title}
          </a>
        ))}
      </nav>

      {domains.map((domain, index) => (
        <section key={domain.id} id={domain.id} className="scroll-mt-28">
          <div className="border-b border-border px-4 py-8 sm:px-8">
            <h2 className="text-2xl font-medium uppercase tracking-tight text-balance md:text-3xl">{domain.title}</h2>
            <p className="mt-3 max-w-2xl text-(--muted) text-pretty">{domain.blurb}</p>
          </div>
          <CardGrid>
            {domain.charts.map((slug) => {
              const chart = bySlug.get(slug)
              if (!chart) return null
              return <ChartCard key={slug} item={{ slug: chart.slug, name: chart.name, tag: chart.tag, family: chart.family }} />
            })}
          </CardGrid>
          {index < domains.length - 1 && <Hatch className="h-6" />}
        </section>
      ))}
    </div>
  )
}
