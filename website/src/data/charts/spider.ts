import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Several measures on spokes around a centre, joined into one polygon per thing you are comparing.",
  intro: [
    "Video game character sheets, football scouting reports and wine tasting cards all do the same thing: take a few measures that have nothing to do with each other, put them on a common 0-to-100 scale and ask you to believe that the resulting shape is a personality. It is a fair trick. A tall, pointy polygon on the right-hand side says *fast and bad at defending* faster than a table of six numbers does.",
    "The spider chart (also called a [radar chart](https://en.wikipedia.org/wiki/Radar_chart), a web chart, or in some circles a star plot) arranges one spoke per measure at equal angles around a centre. Each item gets a point on every spoke, at a distance from the centre that says how high it scores, and the points are joined into a closed polygon. Two polygons laid over each other let you compare two profiles at a glance. People have been drawing versions of this since the nineteenth century, usually under a more dignified name.",
    "It also has the worst reputation in the field, deserved about half the time. The chart is excellent at one job, comparing the *shape* of a handful of profiles, and miserable at nearly everything else. This page is about knowing which half you are in.",
  ],
  math: [
    {
      heading: "Spokes and radius",
      body: [
        "Take $p$ axes (the spokes) and $m$ items. The axes are spread evenly around the circle: axis $a$ is at the angle below, with the first axis at angle zero and the rest following in the order you listed them. An item's value $v_{ia}$ is mapped to a distance from the centre along that spoke.",
        "Every spoke shares one scale. With $M$ the chart's maximum, a value $v$ sits at the fraction below of the outer radius, clamped to $[0, 1]$, so values past the maximum are drawn at the rim rather than off the page and negative values at the centre. Non-finite values are drawn at the centre too. If you do not set `max`, it is the largest finite non-negative value in the whole table, which means the chart rescales when your data changes. Set it when you compare charts.",
      ],
      tex: [
        "\\theta_a = \\frac{2\\pi\\, a}{p}, \\qquad \\rho_{ia} = R \\cdot \\operatorname{clamp}\\!\\left(\\frac{v_{ia}}{M},\\; 0,\\; 1\\right)",
      ],
    },
    {
      heading: "The polygon",
      body: [
        "Item $i$'s polygon joins its $p$ points in axis order, then closes back to the first. By default it is filled at $0.2$ opacity with a $2$ pixel outline, so overlapping shapes stay readable. Grid rings are drawn at $L$ equally spaced radii (five by default), either as polygons that match the axes or, if you choose, as circles.",
      ],
      tex: [
        "P_i = \\bigl( \\rho_{i0}\\cos\\theta_0,\\; \\rho_{i0}\\sin\\theta_0 \\bigr),\\; \\dots,\\; \\bigl( \\rho_{i,p-1}\\cos\\theta_{p-1},\\; \\rho_{i,p-1}\\sin\\theta_{p-1} \\bigr)",
      ],
    },
    {
      heading: "Why the area lies",
      body: [
        "The area of the polygon looks like a summary of how good the item is, and it is not one. For a closed polygon the area is a sum over *neighbouring pairs* of axes, so it depends on the order in which you listed them.",
        "Swap two axes and the same data produces a different area, which a quick check will confirm. Treat the shape as a way to compare profiles, not the area as a score. If you need a single number, compute it yourself and put it in a bar chart.",
      ],
      tex: [
        "\\text{Area}(P_i) = \\frac{\\sin(2\\pi/p)}{2} \\sum_{a=0}^{p-1} \\rho_{ia}\\,\\rho_{i,(a+1) \\bmod p}",
      ],
    },
  ],
  use: [
    "You are comparing **a few items across 3 to 10 measures** that share a scale, such as scores out of 100, skill ratings or percentile ranks.",
    "The *shape* of a profile is the answer: a balanced all-rounder versus a specialist with one high spike.",
    "You want a compact picture of one or two things, a player against the league average, a product against its competitor, that a non-technical reader will take in without a lecture.",
    "The measures are conceptually on the same level and have no natural order, so the arbitrary order of the spokes does not mislead.",
  ],
  avoid: [
    "You have more than about five items. Overlapping polygons blur into a coloured cobweb. Use [parallel coordinates](/charts/parallel-coordinates) or a grouped bar chart.",
    "The measures are on different units and you have not put them on one scale first. The chart has only one scale.",
    "The axes have a natural order, like months or hours. Then you want a line chart, or a [Nightingale rose](/charts/nightingale-rose) if the order is cyclic.",
    "You need to read exact values. Distances from a centre are easy to see and hard to measure. A bar chart is the honest choice.",
  ],
  read: [
    "Find the centre and the rim: the centre is the minimum (zero), the rim is the chart's `max`. Ignore the grid at first and just look at the shapes.",
    "Compare polygons by where they stick out. A spike on one spoke is a strength, a dent is a weakness, and a near-circle is an all-rounder.",
    "Do not read the area. It changes if you change the axis order. Look at individual spokes instead.",
    "Hover a spoke to see each item's value on that axis. Click a legend entry to hide a shape or isolate one.",
  ],
  story: {
    title: "Three left backs and one very honest chart",
    paragraphs: [
      "Dami was the only analyst at a third-division football club, which in practice meant he was also the person who knew where the spare pens were. In January the manager came to his desk with a problem and a coffee: the club could afford one new left back, there were three on the shortlist, and the scouts disagreed about all three.",
      "Dami had a spreadsheet of six numbers for each player, drawn from the league's open data: tackles won, interceptions, progressive passes, crosses completed, dribbles completed and aerial duels won, all converted to percentiles within the position. Eighteen numbers in a grid and a manager who liked pictures.",
      "The question was not who was best, since nobody could agree on what that meant. It was which of the three would fill the *gap* in this particular team, which was short on crossing and long on tackling. That is a question about profiles, and profiles are what the spider chart is for.",
      "He loaded the percentiles into `<Spider>` from bui-charts with the six measures as axes and fixed `max` at 100, so every player used the same scale. Three polygons, one for each candidate, and a fourth in grey for the current left back.",
      "Candidate A was a big green blob that bulged over tackles and aerials and collapsed on crossing, a copy of the man already at the club. Candidate B was a lopsided kite, pointing hard at progressive passes and crosses. Candidate C was fine at everything, which is a kind of compliment and a kind of insult.",
      "The manager pointed at the kite and said \"him\". Dami pointed out the aerial duels were poor, which it was, and they agreed the club could live with that. B played 17 games, delivered eight assists from crosses, and lost the aerial duels in exactly the way the chart had predicted.",
      "Dami got a pen budget for the next season. It was the only recognition the analysis department needed.",
    ],
  },
  pitfalls: [
    "**One scale for all spokes.** There is only a single `max`. Convert measures to a common scale (percentiles, 0-100 scores) before charting, or the biggest unit takes over.",
    "**Order changes the picture.** The same data with the axes shuffled gives a different polygon and a different area. Put related measures next to each other and keep the order the same between charts.",
    "**Default max follows the data.** If you leave `max` unset, two charts with different data will have different outer rings. Set it explicitly when you compare.",
    "**More than five shapes is a cobweb.** Hide some from the legend, or switch to a different chart.",
    "**Values past the max are clipped at the rim.** A score of 130 on a 100 scale looks the same as 100.",
  ],
  related: [
    { slug: "parallel-coordinates", why: "When you have many items, or measures with different ranges, and want each on its own axis." },
    { slug: "nightingale-rose", why: "When the spokes are a cycle like months and you want wedges that stack by cause." },
    { slug: "bullet", why: "When you only need one measure against its target and range." },
  ],
  media: [
    { kind: "youtube", id: "Nit8oaKthsI", title: "Data Visualization Done Right! - Radar Chart", channel: "Ernesto Flores", note: "Discusses how to use a radar chart well. Made with another tool." },
  ],
}
