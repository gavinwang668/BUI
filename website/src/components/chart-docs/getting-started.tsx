import { TriangleLine } from "./zigzag"
import { Code } from "./code"
import snippets from "@/data/charts/generated/snippets.json"
import { Hatch, Section } from "./hatch"
import { baseCharts, charts, derivedCharts, exampleCount, imageOf } from "@/data/charts/registry"
import { href } from "@/lib/href"

export const metadata = {
  title: "Getting started",
  description: "Install BUI Charts, draw your first chart in an rsx! block, and learn how this documentation is organised.",
}

const shared = [
  [
    "Your data, your shape",
    "Pass a `Vec<T>`, a `Signal<Vec<T>>` or anything that implements `Dataset`. Charts read your rows through small closures like `x={|r| r.load}`, so you never reshape data for the chart.",
  ],
  [
    "Hover, legends and tooltips",
    "Hovering fades the rest and opens a tooltip. Clicking a legend entry hides that series and the chart re-scales. Every chart takes `tooltip`, `legend` and `class`.",
  ],
  ["Zoom and select", "Wheel and pinch zoom the plots that have axes. Drag to select a range where it makes sense."],
  ["Theming", "Charts read the colours of the active theme, so light, dark and your own palette all work without extra props."],
  [
    "Cheap to redraw",
    "Layouts are cached and only recomputed when the data, the size, the hidden set or the theme changes. Hovering does not re-layout.",
  ],
] as const

export default function GettingStarted() {
  const picks = ["mekko", "sankey", "heatmap", "candlestick", "flame-graph", "ternary"].map((slug) => charts.find((chart) => chart.slug === slug)!)

  return (
    <div className="charts-doc">
      <header className="border-b border-border px-4 pb-8 pt-8 sm:px-8 md:pt-12">
        <p className="mb-4 text-xs font-semibold uppercase tracking-[0.16em] text-primary">Getting started</p>
        <h1 className="text-4xl font-light uppercase tracking-tight text-balance md:text-6xl">Charts that make your mouth drool</h1>
        <p className="mt-5 max-w-2xl text-lg text-pretty md:text-xl">
          <span className="tabular">{charts.length}</span> charts, <span className="tabular">{exampleCount}</span> runnable examples, BUI charts
          give you creative control over your data.
        </p>
        <TriangleLine className="mt-8 w-full max-w-md" height={14} peakWidth={20} strokeWidth={2} />
      </header>

      <Section id="install" title="Install" kicker="One minute">
        <div className="flex max-w-2xl flex-col gap-4 text-base leading-7 md:text-[17px] md:leading-8">
          <p>
            BUI Charts is a set of components for{" "}
            <a href="https://github.com/ppmpreetham/BUI" target="_blank" rel="noopener noreferrer" className="prose-link">
              BUI
            </a>
            . Run this command below to use it in your project.
          </p>
          <Code label="Terminal" html={snippets.install} />
        </div>
      </Section>

      <Section id="first" title="Your first chart" kicker="Two minutes">
        <div className="flex max-w-2xl flex-col gap-4 text-base leading-7 md:text-[17px] md:leading-8">
          <p>
            A chart is a component. Write it in <code>rsx!</code> like any other tag, style it with <code>class</code>, and feed it from your own
            state. When the signal changes, the chart redraws.
          </p>
          <Code label="src/dashboard.rs" html={snippets.first} />
        </div>
      </Section>

      <Hatch className="h-8" />

      <Section id="shared" title="What every chart shares" kicker="Good to know">
        <dl className="grid gap-px overflow-hidden border border-border bg-border md:grid-cols-2">
          {shared.map(([term, text]) => (
            <div key={term} className="bg-background p-5">
              <dt className="mb-2 text-sm font-semibold uppercase tracking-[0.08em]">{term}</dt>
              <dd className="text-(--muted)" dangerouslySetInnerHTML={{ __html: text.replace(/`([^`]+)`/g, "<code>$1</code>") }} />
            </div>
          ))}
        </dl>
      </Section>

      <Section id="docs" title="How these docs work" kicker="Orientation">
        <div className="flex max-w-2xl flex-col gap-4 text-base leading-7 md:text-[17px] md:leading-8">
          <p>
            There are <span className="tabular">{baseCharts.length}</span> base charts, the ones everything else is built on, and{" "}
            <span className="tabular">{derivedCharts.length}</span> derived charts that can help you visualize your data.
          </p>
          <p>Since there are a lot of charts, here are a few pages you should probably visit next:</p>
        </div>
        <div className="mt-8 grid gap-px overflow-hidden border border-border bg-border md:grid-cols-3">
          {[
            { href: href("charts/all"), title: "All charts", text: "Every chart as a picture. Filter by name or family." },
            { href: href("charts/which"), title: "Which chart to use?", text: "Start from the question you want to answer." },
            { href: href("charts/domains"), title: "Domain specific", text: "Finance, electronics, biology, earth science and more." },
          ].map((card) => (
            <a
              key={card.href}
              href={card.href}
              className="group flex flex-col gap-2 bg-background p-5 transition-[background-color,color] duration-150 ease-out hover:bg-primary hover:text-white focus-visible:bg-primary focus-visible:text-white"
            >
              <span className="text-sm font-semibold uppercase tracking-[0.08em]">{card.title}</span>
              <span className="text-sm text-(--muted) group-hover:text-white/90">{card.text}</span>
            </a>
          ))}
        </div>
      </Section>

      <Section id="start-with" title="Or just start with one of these" kicker="Pick a door">
        <div className="-mx-4 grid gap-px border-y border-border bg-border sm:-mx-8 sm:grid-cols-2 xl:grid-cols-3">
          {picks.map((chart) => (
            <a key={chart.slug} href={href(`charts/${chart.slug}`)} className="group flex flex-col gap-3 bg-background p-4">
              <img
                src={imageOf(chart.slug)}
                alt={`${chart.name} preview`}
                width={480}
                height={270}
                loading="lazy"
                className="img-outline aspect-video w-full object-cover transition-[scale] duration-300 ease-out group-hover:scale-[1.02]"
              />
              <span className="text-sm font-semibold uppercase tracking-[0.08em] group-hover:underline group-hover:underline-offset-4">
                {chart.name}
              </span>
            </a>
          ))}
        </div>
      </Section>
    </div>
  )
}
