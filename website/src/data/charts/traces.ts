import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One request, drawn as nested bars across every service it touched.",
  intro: [
    "A user clicks Pay. Forty milliseconds later a spinner is still spinning, and somewhere between the gateway, the cart service, the fraud check and a database that everyone has agreed not to discuss, the time went missing. A log file can tell you what each service did. It cannot tell you what they did *in order*, or while waiting for whom.",
    "A distributed trace fixes that by recording each unit of work as a **span**: a service name, an operation, a start time, a duration, and the id of the span that caused it. Put the spans on a shared clock, indent each under its parent, and you get the picture this chart draws. It descends from Google's [Dapper paper](https://research.google/pubs/dapper-a-large-scale-distributed-systems-tracing-infrastructure/), and from [Zipkin](https://zipkin.io/) and [Jaeger](https://www.jaegertracing.io/), and today from [OpenTelemetry](https://opentelemetry.io/). The visual idiom, the waterfall of bars with a tree on the left, has barely changed, because it works.",
    "The chart reads like a [Gantt chart](https://en.wikipedia.org/wiki/Gantt_chart) that has had a bad day. Long bars are slow things. Bars that start after another ends are things that waited. A gap between a parent and its first child is time spent doing nothing in particular, which is the most expensive kind.",
  ],
  math: [
    {
      heading: "Spans, parents and the tree",
      body: [
        "Each span $s$ has a start $t_s$ in microseconds and a duration $d_s$, so it covers $[t_s,\\, t_s + d_s]$. Spans with a start of 0 are dropped. A span's parent is the span named by its `parent` field; if there is none, the span under its first reference is used (a \"follows from\" link). Spans whose parent is not in the trace are placed at the top level.",
        "Children are ordered by start time and the tree is walked depth first, which gives the row order and the indentation depth. The trace as a whole runs from the earliest start to the latest end.",
      ],
      tex: [
        "T = \\max_s (t_s + d_s) \\;-\\; \\min_s t_s",
      ],
    },
    {
      heading: "From time to pixels",
      body: [
        "The timeline column is a linear scale. If the visible window is $[w_0, w_1]$ (the whole trace until you drag across the overview to zoom) and the bar area is $W$ pixels wide, a span is drawn at the position and width below. The label on the bar is its duration by default, or its percentage of the trace, or none.",
      ],
      tex: [
        "x_s = W\\,\\frac{t_s - w_0}{w_1 - w_0}, \\qquad \\ell_s = W\\,\\frac{d_s}{w_1 - w_0}",
      ],
    },
    {
      heading: "Critical path",
      body: [
        "The critical path answers the question \"if I made only this one thing faster, would the request be faster?\" It is found by walking backwards from the end of the root span. At each span, take the child that finishes last before the current point. The time between that child's end and the current point belongs to the parent, so it is added as a section of the parent's critical path. Then continue into that child. When a span has no such child left, its own remaining time from its start is added and the walk returns to its parent, resuming from where the child began.",
        "Two details keep this honest. A child is first clipped to its parent's interval, and a child that lies entirely outside it is ignored. Spans linked only by a reference (not a parent) are never on the path. The result is a set of time sections, not whole spans: a span can be partly critical, and the chart shades exactly those parts.",
      ],
    },
  ],
  use: [
    "You are chasing latency in a system with more than two services and need to know where the time went in one particular request.",
    "You have a trace export from Jaeger or OpenTelemetry and want to look at it without standing up a backend. `Trace::from_jaeger_json` and `from_otlp_json` read both.",
    "You want to separate work that is slow from work that is merely waiting, using the critical path filter.",
    "You are debugging errors and want to see which span failed first, with its tags, logs and stack trace.",
  ],
  avoid: [
    "You want to know how latency is distributed across thousands of requests. One trace is one anecdote. Use a [Histogram](/charts/histogram) or a [Heatmap](/charts/heatmap).",
    "You want to see which services call which, regardless of any one request. That is a [Node Graph](/charts/node-graph).",
    "You want to see where CPU time goes inside one process. That is a [Flame Graph](/charts/flame-graph), built from samples rather than spans.",
    "The trace has tens of thousands of spans and you are hoping to see all of them at once. Filter first, then read.",
  ],
  read: [
    "Read the header: the root operation, total duration, the number of services and spans, and the banner pointing at the most telling error.",
    "Scan the bars top to bottom. Indentation shows who called whom, and bar position shows when. A staircase of bars is a chain of sequential calls. Bars stacked at the same time are parallel.",
    "Turn on the Critical path pill. Whatever stays highlighted is what the request was actually waiting for.",
    "Click a span to open its details: tags, process attributes, logs, warnings and stack trace. Drag across the overview to zoom, and use the filter bar (service, duration, tags, text) to hide what you do not care about.",
  ],
  story: {
    title: "The 900 milliseconds that were 40",
    paragraphs: [
      "Ines was the on-call engineer for an online ticketing site, a job that mostly consisted of being awake on the day a band announced a tour. The checkout endpoint had a p95 of 1.4 seconds. The product team wanted it under 600 ms by the next on-sale, which was eleven days away and, in the way of these things, already sold out in people's imaginations.",
      "Everyone had a theory. The database was slow. The payment provider was slow. The seat-hold service was written in a hurry and, said one colleague, *had it coming*. Each theory had a graph to support it, and the graphs were all from different services and different hours.",
      "What Ines needed was not another graph. She needed one slow request, taken apart, so she could see what the checkout was actually waiting for and for how long.",
      "She exported a trace of a 1.3 second checkout from her tracing backend, loaded it into a small desktop tool with `Trace::from_otlp_json`, and put it on screen with `<Traces>`. The tree had 61 spans across seven services. The database spans, the prime suspect, were short thin slivers.",
      "She switched on the critical path. Most of the bars dimmed. What remained lit was a long block in the seat-hold service, and inside it a run of four calls to the pricing service, one after another, each about 180 ms. They had no data dependency. Somebody had written them in a loop.",
      "The fix was to send the four requests together. The checkout dropped from 1.3 seconds to 480 ms in the test environment, p95 landed at 540 ms after release, and the loop's author, to their credit, framed the trace and hung it above their desk.",
      "The database was never mentioned in the post-mortem again. It was, in the end, merely very quiet.",
    ],
  },
  pitfalls: [
    "**Clock skew.** Spans from different machines are placed by their own clocks. If a child appears to start before its parent, the machines disagree. The chart draws what it was told.",
    "**Missing parents.** A span whose parent was never recorded shows up at the top level, not under the right caller. That is a sampling or instrumentation gap, not a chart bug.",
    "**One trace is not the system.** A trace you picked because it was slow tells you why that request was slow. Check that the pattern repeats before changing the architecture.",
    "**Critical path is about time, not cost.** A short span on the path that is called ten thousand times a day may matter more than a long one that happens once.",
    "**Zero start times.** Spans with a start of 0 are dropped, so an exporter that leaves the field empty will produce an empty chart.",
  ],
  related: [
    { slug: "node-graph", why: "When you want the shape of the whole system, not one request through it." },
    { slug: "flame-graph", why: "When the time is inside one process and you have samples instead of spans." },
    { slug: "gantt", why: "When the bars are planned work rather than recorded work." },
  ],
  media: [
    { kind: "youtube", id: "0B8YEAQHlP0", title: "Trace Visualization #distributedtracing #grafana #grafanatempo #observability #oneminuteo11y", channel: "Datahovel", note: "A short look at a distributed trace visualization in Grafana Tempo, the kind of view this chart draws." },
    { kind: "youtube", id: "0tlp7QCPu0k", title: "What metrics and logs don’t show you (that traces do) #observability", channel: "Grafana", note: "Grafana on what traces reveal that metrics and logs do not." },
  ],
}
