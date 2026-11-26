import { PageHead } from "../addon/page-head"
import { loadDoc } from "@/data/charts/docs"
import { charts, exampleCount } from "@/data/charts/registry"

export const metadata = {
  title: "All charts",
  description: "Every chart in BUI Charts at a glance. Pick one to open its page.",
}

/** The <AllGrid> island's props: every chart with its tagline, and the families. */
export async function allGridProps() {
  const docs = await Promise.all(charts.map((chart) => loadDoc(chart.slug)))
  const items = charts.map((chart, index) => ({
    slug: chart.slug,
    name: chart.name,
    tag: chart.tag,
    family: chart.family,
    group: chart.group,
    tagline: docs[index]?.tagline,
  }))
  const familyNames = [...new Set(charts.map((chart) => chart.family))]
  return { items, families: familyNames }
}

/** `grid` is the <AllGrid> island, which the Astro page hydrates. */
export default function AllPage({ grid }: { grid?: React.ReactNode }) {

  return (
    <div className="charts-doc">
      <PageHead kicker="Overview" title="All charts">
        <p className="mt-5 max-w-2xl text-lg text-pretty text-(--muted)">
          <span className="tabular">{charts.length}</span> charts and <span className="tabular">{exampleCount}</span> examples. These are still pictures so the page opens quickly. The live versions wait for you on each chart page, and the Examples, Math and API links jump straight to that section.
        </p>
      </PageHead>
      {grid}
    </div>
  )
}
