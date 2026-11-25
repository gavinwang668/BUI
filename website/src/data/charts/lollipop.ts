import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A bar chart with the bars replaced by a thin stick and a dot, for many items or long names.",
  intro: [
    "A bar chart with forty bars is a wall. The ink adds up, the bars crowd one another, and the eye drifts off to find the legend, which is also a wall. The lollipop chart is what you get when you keep the one thing a bar encodes (a length from zero) and throw out most of the paint.",
    "Each item gets a thin line from the baseline and a dot at the end. The dot is where the value is, and the stick is just there to remind you it started at zero. The result carries the same information as a bar chart with far less visual weight, so you can fit twice as many items and still read the labels.",
    "It is sometimes accused of being a bar chart in a nicer coat, and that is more or less true. The coat matters. Past about fifteen items, a stack of fat bars turns into a pattern, and a pattern is a bad thing to read a number from.",
  ],
  math: [
    {
      heading: "A bar, reduced",
      body: [
        "Given $n$ rows with names $c_i$ and values $v_i$, the chart orders them by `sort` and assigns each one a slot $0, 1, \\dots, n-1$ along the category axis. With `sort` set to none the order is yours; ascending or descending sorts by $v$.",
        "The value axis is an ordinary linear axis. The stick is a bar of width 5% of its slot, capped at 2 pixels, drawn from the baseline to $v_i$. The dot is a marker of `point_size` pixels (12 by default) at the same value, in the same colour.",
      ],
      tex: ["\\text{slot}(i) = \\operatorname{rank}_{\\text{sort}}(v_i), \\qquad \\text{stick} = [\\,0,\\ v_i\\,], \\qquad \\text{dot} = v_i"],
    },
    {
      heading: "Why the stick stays",
      body: [
        "A dot alone is a [dot plot](https://en.wikipedia.org/wiki/Dot_plot_(statistics)), and it is honest about position but loose about the baseline. The stick brings back the baseline, which is what makes the length comparable between items. It is hidden from the legend, because it is a decoration for the dot, not a series of its own.",
        "The chart defaults to horizontal sticks (`horizontal={true}`) so that long names run down the left side and can be read without tilting your head. Value labels next to the dots are off by default. Turn them on with `show_values`.",
      ],
    },
  ],
  use: [
    "You have many items, say 15 to 60, and a bar chart would be a dense block of colour.",
    "Category names are long: product names, file paths, survey questions. Horizontal sticks leave them room.",
    "You want a ranked list that reads as a ranking: sort it descending and the dots form a clean slope.",
  ],
  avoid: [
    "You have three or four items. A plain bar is bolder and perfectly readable.",
    "The values can be negative and large. A stick from zero to a negative dot works, but the picture gets confusing quickly. A [back-to-back](/charts/back-to-back) or [waterfall](/charts/waterfall) is often clearer.",
    "You need to compare two values per item. Use a [dumbbell](/charts/dumbbell), which draws both on one stick.",
    "The axis does not start at zero and you cannot make it. The stick implies a baseline, so a cut axis turns the length into a lie.",
  ],
  read: [
    "Read position along the value axis, not stick length. The dot is the number.",
    "Run your eye down the dots. With a sorted chart you are looking at the shape of the distribution: a steep start and a long tail is the usual story.",
    "Look for gaps between neighbours. A big jump between two adjacent items is where the interesting break is.",
    "Hover a dot for the exact name and value.",
  ],
  story: {
    title: "Fifty-two ways to be late",
    paragraphs: [
      "Hana worked in operations for a regional rail operator, and her quarterly job was to explain to the board which stations caused the most delay minutes. There were fifty-two stations. She had tried a bar chart. It was a very accurate bar chart, in the sense that it was accurate and nobody could read it.",
      "The station names were the real trouble. Names like \"Northgate Interchange (Platform 4 Terminal)\" had to be rotated to fit under their bars, and rotated text is read exactly as often as it deserves to be. The chairman had been seen tilting his head at the slide like a spaniel.",
      "What Hana wanted was the ranking: which dozen stations account for most of the delay, and where does it fall off. The thickness of the bars was working against her, because fifty-two thick bars look like a barcode.",
      "She switched to `<Lollipop>` with the station name and delay minutes, sorted descending, horizontal by default. The names now ran down the left in normal type. The dots slid from top left to bottom right along a curve that anyone could read in two seconds.",
      "The curve had a shoulder. The first eight stations were above 4,000 minutes each, the next forty-four mostly below 900. Hana put the figure in the deck with a single sentence above it and no further slides about the long tail.",
      "The board funded platform work at six of the top eight, and the fourth-ranked station turned out to be waiting on a points failure that had been logged in a ticket nobody opened. Delay minutes on those eight fell by about a third over the next two quarters.",
      "A good chart makes the long tail look as short as it deserves to. This one let the tail trail off gracefully instead of shouting.",
    ],
  },
  pitfalls: [
    "**Truncated axes.** The stick suggests a zero baseline. If the axis starts at 40, a dot at 60 looks twice as far as a dot at 50 when it is only 1.5 times.",
    "**Unsorted lists.** Alphabetical order hides the ranking. Pass `sort` unless the order itself means something, like months.",
    "**Dots that are too big.** At 12 pixels the dots can touch in dense charts. Reduce `point_size` instead of increasing the chart height.",
    "**Using it for two series.** There is only one series per chart. For before and after, use [dumbbell](/charts/dumbbell) or [slope](/charts/slope).",
  ],
  related: [
    { slug: "dumbbell", why: "When each item has two values and the gap between them is the point." },
    { slug: "bar-gauge", why: "When each bar should show how full something is against a minimum and a maximum." },
    { slug: "pareto", why: "When the ranking should come with a cumulative line showing the vital few." },
  ],
  media: [
    {kind:"youtube",id:"huBhqjAXsT4",title:"Tableau Explained: How to Create a Lollipop Chart",channel:"Andre de Vries",note:"Step-by-step build of a lollipop chart in Tableau, showing when it beats a bar chart."},
    {kind:"youtube",id:"4Jnb5jPjJqA",title:"#Tableau - Lollipop Chart",channel:"Andy Kriebel",note:"Quick demonstration of a lollipop chart built in Tableau."},
  ],
}
