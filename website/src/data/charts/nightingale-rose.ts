import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A polar bar chart: one wedge per period or direction, stacked by cause, with area that follows the value.",
  intro: [
    "In 1858 [Florence Nightingale](https://en.wikipedia.org/wiki/Florence_Nightingale) had a problem that was not medical. The British Army had lost far more men to disease in the Crimean War than to the enemy, and the people who could change that, ministers and committee members, did not read tables. She drew them a picture instead: a ring of wedges, one for each month, with the area of each wedge standing for the number of deaths. The big blue wedges were deaths from preventable disease, and they dwarfed everything else.",
    "She called them wedges. History has called them coxcombs, polar area diagrams and, most often, Nightingale roses. Whatever the name, the form is a bar chart bent around a circle. Each period gets a sector, the sectors stack from the centre outward by cause, and because the circle is the natural shape of a year or a day or a compass, the picture shows the cycle as well as the size.",
    "A rose is for data that repeats. Twelve months, twenty-four hours, eight wind directions: things that come back to where they started. Put those on a line and you cut the cycle at an arbitrary point. Wrap them around a circle and January sits next to December, which is the only honest place for it.",
  ],
  math: [
    {
      heading: "Sectors and stacks",
      body: [
        "Take $n$ sectors (months, say) and $m$ series (the causes). Sector $j$ is centred at the angle below and spans $2\\pi/n$ with a small gap on either side, and the first sector starts half a step round, so no sector is centred on the zero direction. Series stack from the centre outward in the order you give them: the first series is innermost.",
        "Let $v_{ij}$ be series $i$ in sector $j$. Only finite positive values are drawn, and the cumulative value at the outer edge of series $i$ is the sum up to and including it.",
      ],
      tex: [
        "\\theta_j = \\frac{2\\pi\\,(j + \\tfrac12)}{n}, \\qquad C_{ij} = \\sum_{k \\le i} v_{kj}",
      ],
    },
    {
      heading: "Radius follows the square root",
      body: [
        "This is the point of the chart, and it is the point Nightingale got right. If radius were proportional to value, a wedge that is twice the value would have *four times* the area, since area grows with the square of the radius, and the eye would overreact. So the radius uses the square root of the cumulative value, and the area of the ring segment is proportional to the value it stands for.",
        "With $R$ the outer radius and $M$ the chart's maximum, the boundary after series $i$ in sector $j$ is at the radius below, clamped between $0$ and $R$. If you do not set `max`, it is the tallest *stacked* column, that is the largest sum of values within any one sector.",
      ],
      tex: [
        "r_{ij} = R \\sqrt{\\operatorname{clamp}\\!\\left(\\frac{C_{ij}}{M},\\; 0,\\; 1\\right)}",
        "\\text{area}(i,j) = \\frac{\\pi}{n}\\,\\bigl(r_{ij}^2 - r_{i-1,j}^2\\bigr) \\;\\propto\\; v_{ij}",
      ],
    },
    {
      heading: "What the rings mean",
      body: [
        "The faint reference rings are drawn at equal steps of *radius*, four by default, not at equal steps of value. Because radius is a square root of value, the rings are closer together in value terms near the rim. Use them to judge how far out a wedge goes, and use hover for the numbers. A wedge twice as big in value is about $1.41$ times as long, and the chart is honest about it.",
      ],
    },
  ],
  use: [
    "The categories are **cyclic**: months, hours of the day, days of the week, compass directions. The wrap-around is the point.",
    "You want a composition inside each period too, such as deaths by cause, sales by channel, rainfall by storm type.",
    "You have between six and twenty-four sectors. Fewer is a pie. More is a hedgehog.",
    "Your audience is more likely to remember a picture than a table, and the story is about a cause that grows and shrinks through the cycle.",
  ],
  avoid: [
    "The categories are not cyclic. Use a bar chart: it is easier to read and does not pretend the first and last are neighbours.",
    "You need accurate comparison between sectors. Lengths on a radial layout are harder to judge than lengths on a straight baseline.",
    "There is one wedge much bigger than the rest. Because the radius is a square root, it compresses the large values, which is honest and sometimes disappointing.",
    "You have many series. Beyond four or five, stacked wedges become a shuffle of thin slices.",
  ],
  read: [
    "Find the starting sector and its direction. The sectors are in the order you gave them, going once round the circle.",
    "Judge each wedge by its *area*. Do not compare radii directly: a wedge that reaches twice as far holds about four times the value.",
    "Read the stack from the centre outward. The innermost colour is the first series. Its share of a sector is easiest to judge because everything else stacks on it.",
    "Look for the sectors that jut out. Those are the peaks of the cycle, and the colours show what causes them.",
    "Hover a sector for its values. Click a legend entry to hide a series or isolate one.",
  ],
  story: {
    title: "The month that broke the pie",
    paragraphs: [
      "Hamid ran operations for a regional pharmacy chain, forty shops and one head office whose coffee machine had been broken since the previous June. His quarterly job was to explain staffing, and his quarterly obstacle was that his bosses read two kinds of charts: line charts that went up, and anything he did not show them.",
      "The thing he needed to explain was call volume to the pharmacy helpline, which came in three flavours: refill questions, side-effect questions and stock questions. The monthly totals had a pattern, but it was a pattern buried in a twelve-by-three table and the head of customer service had no appetite for twelve-by-three tables.",
      "What he needed was a picture where December and January were visibly neighbours, because the winter peak ran across the new year, and where each month's calls were split by type so the cause of the peak was part of the picture, not a footnote. A line chart cut the winter in half. A stacked bar did the same.",
      "He put the 36 numbers into `<NightingaleRose>` from bui-charts with twelve months as `sectors` and three series. Refills went in first, so they sat on the inside. The chart rounded the year into a clock face with a bulge.",
      "The bulge was side-effect questions. They did not peak in winter, as everyone assumed, but in late September and October, in a fat orange crescent about a month wide that had nothing to do with the cold. A new brand of blood-pressure tablets had come into stock in September, and the helpline had been absorbing the questions without anyone noticing.",
      "Hamid took the picture to the pharmacists, who agreed to put a printed leaflet in every bag. The following autumn the crescent shrank by about 40% and the helpline staff took their first full lunch break in October.",
      "Florence Nightingale would have recognised the diagram and been unimpressed by the coffee machine.",
    ],
  },
  pitfalls: [
    "**Read area, not radius.** Because radius is the square root of the stacked value, a wedge that is twice the value is about 1.4 times as long. Judge by area.",
    "**Series order changes the look.** The first series is innermost and easiest to measure. Put the one you care most about first.",
    "**Equal radius steps, not equal value steps.** The faint rings are evenly spaced in radius. They are a guide, not a scale, so use hover for numbers.",
    "**Negative and zero values are not drawn.** Only finite positive values make a wedge.",
    "**Default max follows the tallest sector.** If you compare two roses, set `max` yourself so they share a scale.",
  ],
  related: [
    { slug: "spider", why: "When the spokes are not a cycle and you want to compare profiles, not stack causes." },
    { slug: "sunburst", why: "When the stack inside each wedge is itself a hierarchy." },
    { slug: "mekko", why: "When the periods are not cyclic and you want area to carry the answer in a rectangle." },
  ],
  media: [
    {
      kind: "youtube",
      id: "JZh8tUy_bnM",
      title: "Florence Nightingale's Famous Rose Chart (aka \"Coxcomb\")",
      channel: "eagereyes – Robert Kosara",
      note: "A visualization researcher walks through what Nightingale's chart actually shows and why it is more than a pretty picture.",
    },
  ],
}
