import { PageHead } from "../addon/page-head"
import { CardGrid, ChartCard } from "./chart-card"
import { Hatch } from "./hatch"
import { bySlug } from "@/data/charts/registry"
import { href } from "@/lib/href"
import { intents } from "@/data/charts/intents"

export const metadata = {
  title: "Which chart to use?",
  description: "Start from the question you want to answer and find the chart that answers it.",
}

export default function WhichPage() {
  return (
    <div className="charts-doc">
      <PageHead kicker="Choosing" title="Which chart to use?">
        <p className="mt-5 max-w-2xl text-lg text-pretty text-(--muted)">
          Do not start from the chart. Start from the sentence you want the reader to say out loud after looking at it. Find that sentence below, and every chart that can help is one click away.
        </p>
      </PageHead>

      <nav aria-label="Questions" className="flex flex-wrap gap-2 border-b border-border p-4 sm:px-8">
        {intents.map((intent) => (
          <a
            key={intent.id}
            href={`#${intent.id}`}
            className="inline-flex min-h-10 items-center rounded-full border border-border px-4 text-sm transition-[background-color,color,scale] duration-150 ease-out hover:bg-primary hover:text-white active:scale-[0.96]"
          >
            {intent.question.replace(/^I (want to |work with |study |am |)/, "").replace(/^./, (letter) => letter.toUpperCase())}
          </a>
        ))}
      </nav>

      {intents.map((intent, index) => (
        <section key={intent.id} id={intent.id} className="scroll-mt-28">
          <div className="border-b border-border px-4 py-8 sm:px-8">
            <p className="tabular mb-2 text-xs font-semibold uppercase tracking-[0.16em] text-primary">{String(index + 1).padStart(2, "0")}</p>
            <h2 className="text-2xl font-medium tracking-tight text-balance md:text-3xl">{intent.question}</h2>
            <p className="mt-3 max-w-2xl text-(--muted) text-pretty">{intent.blurb}</p>
          </div>
          <CardGrid>
            {intent.picks.map((pick) => {
              const chart = bySlug.get(pick.slug)
              if (!chart) return null
              return <ChartCard key={pick.slug} item={{ slug: chart.slug, name: chart.name, tag: chart.tag, family: chart.family, note: pick.when }} />
            })}
          </CardGrid>
          {index < intents.length - 1 && <Hatch className="h-6" />}
        </section>
      ))}

      <div className="p-8 text-center text-sm text-(--muted)">
        Still undecided? Browse <a href={href("charts/all")} className="underline underline-offset-4">all charts</a> or look at them <a href={href("charts/domains")} className="underline underline-offset-4">by domain</a>.
      </div>
    </div>
  )
}
