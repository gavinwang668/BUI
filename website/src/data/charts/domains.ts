export type Domain = {
  id: string
  title: string
  blurb: string
  charts: string[]
}

export const domains: Domain[] = [
  {
    id: "operations",
    title: "Operations and reliability",
    blurb: "You run something that has to stay up, and you want to see what it is doing right now and what it did at 3 a.m.",
    charts: ["time-series", "trend", "heatmap", "histogram", "state-timeline", "status-history", "flame-graph", "icicle", "traces", "node-graph", "honeycomb", "stat", "gauge", "bar-gauge", "gantt"],
  },
  {
    id: "finance",
    title: "Finance and trading",
    blurb: "Prices, returns, risk and the quarterly meeting where someone asks why the number moved.",
    charts: ["candlestick", "heikin-ashi", "renko", "kagi", "point-figure", "efficient-frontier", "waterfall", "horizon", "mekko", "bump", "slope", "dumbbell"],
  },
  {
    id: "manufacturing",
    title: "Manufacturing and quality",
    blurb: "Process control, defects and finding the one machine that is quietly ruining your Tuesday.",
    charts: ["control-chart", "pareto", "ishikawa", "histogram", "bland-altman", "bullet", "gauge", "bar-gauge", "state-timeline", "oscilloscope"],
  },
  {
    id: "product",
    title: "Product and marketing analytics",
    blurb: "Funnels, journeys, segments and the slow realisation that nobody reads the dashboard.",
    charts: ["funnel", "sankey", "sankey-funnel", "sankey-matrix", "alluvial", "mekko", "waffle", "word-cloud", "lollipop", "back-to-back", "slope", "dumbbell", "bump", "upset", "venn"],
  },
  {
    id: "statistics",
    title: "Statistics and research",
    blurb: "Distributions, agreement between methods, effect sizes and honest uncertainty.",
    charts: ["histogram", "barcode", "beeswarm", "ridgeline", "horizon", "forest", "bland-altman", "dendrogram", "parallel-coordinates", "attention-map", "venn", "upset", "efficient-frontier"],
  },
  {
    id: "electronics",
    title: "Electronics, signals and control",
    blurb: "Waveforms, spectra, links that must stay open, and loops that must stay stable.",
    charts: ["oscilloscope", "spectrogram", "eye-diagram", "smith-chart", "root-locus", "radiation-pattern", "hodograph", "phase-portrait", "poincare-map", "bifurcation", "quiver"],
  },
  {
    id: "life-science",
    title: "Biology and chemistry",
    blurb: "Genomes, protein shapes, conserved letters and the chemistry of water.",
    charts: ["sequence-logo", "ramachandran", "circos", "upset", "venn", "dendrogram", "ternary", "piper"],
  },
  {
    id: "earth",
    title: "Earth, weather and geology",
    blurb: "Soundings, rocks, groundwater and maps where the colours stand for something.",
    charts: ["stuve", "tephigram", "ternary", "ternary-contour", "piper", "harker", "choropleth"],
  },
  {
    id: "physics",
    title: "Physics and astronomy",
    blurb: "Stars, orbits, chaos and what happens when you nudge a system and watch.",
    charts: ["h-r-diagram", "phase-portrait", "poincare-map", "bifurcation", "quiver", "hodograph", "radiation-pattern"],
  },
  {
    id: "teams",
    title: "Projects, people and structure",
    blurb: "Who depends on whom, what is the plan and where do the arrows cross.",
    charts: ["gantt", "ishikawa", "chord-diagram", "arc-diagram", "treemap-force", "sunburst", "icicle", "dendrogram", "nightingale-rose", "spider", "sankey"],
  },
]
