import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A hierarchy as concentric rings, where each slice's angle is its share of its parent.",
  intro: [
    "A [pie chart](https://en.wikipedia.org/wiki/Pie_chart) has one level. That is both its charm and its problem: ask it about a company's revenue by region, and it is fine. Ask it about revenue by region, then product, then plan, and you are suddenly looking at three pies and a headache. A sunburst is what you get when you decide that a pie chart should have children.",
    "The centre is the whole. The first ring splits it into its top-level parts, the way a normal pie would. The second ring splits each of those parts in turn, directly outside its parent and within the same angular span, and so on, ring after ring. A slice's angle is its share of the parent, and its position tells you what it belongs to. The form goes back to John Stasko's work on space-filling hierarchy displays around 2000, and became popular for exactly one reason: it fits a lot of structure into a circle and still looks good on a poster.",
    "In bui-charts it is also clickable. Click a slice and it becomes the new centre, with its descendants filling the rings. Click the centre to climb back out. That is the difference between a nice picture and a thing you can actually explore.",
  ],
  math: [
    {
      heading: "Paths become a tree",
      body: [
        "Each row gives a **path** (a string such as `Europe/Spain/Madrid`) and a value. The path is split on the separator, which defaults to `/`, each piece is trimmed, and empty pieces are dropped. Pieces with the same name under the same parent are the same node, so rows pile up on shared prefixes. Rows with a non-finite or non-positive value are skipped.",
        "The value is added to the node at the *end* of the path. Then totals are pushed up from the leaves, so every node holds its own value plus everything below it. The root, called `Total` unless you rename it, is the grand total. A parent can carry a value of its own (a path that is a prefix of another path), and the part not covered by its children shows up as a gap in the ring.",
      ],
      tex: [
        "T(v) = a(v) + \\sum_{c \\,\\in\\, \\text{children}(v)} T(c)",
      ],
    },
    {
      heading: "Angles and radii",
      body: [
        "Take a focused node (the root until you click) whose slice spans the angles $[\\theta_0, \\theta_1]$. Its children divide that span in proportion to their totals, in the order they first appeared in the data. The angle of child $c$ is its share of the parent, so full circles are $2\\pi$ at the root and smaller wedges deeper down.",
        "Rings are all the same thickness. The outer radius $R$ is half the smaller side of the chart less a few pixels, the hole in the middle is $0.2\\,R$, and the remaining space is divided evenly among the $D$ visible levels, where $D$ is the height of the focused subtree, capped by the `levels` prop if you set it. A wedge too thin to see (under about $0.002$ radians) is not drawn at all.",
      ],
      tex: [
        "\\Delta\\theta(c) = \\frac{T(c)}{T(\\text{parent})}\\,\\Delta\\theta(\\text{parent}), \\qquad r_k = 0.2R + \\frac{0.8R}{D}\\,k",
      ],
    },
    {
      heading: "Colour and labels",
      body: [
        "Colour comes from the *top-level branch* a node lives in, taken from the chart palette, and each deeper ring is lightened by $0.07$ in lightness (capped so it never goes white). That means a whole family is one hue, graded from dark at the middle to pale at the rim.",
        "A name is printed inside a wedge only if the arc length at the middle of its ring is wider than the text plus a margin, so small slices stay quiet. Hover shows the node's value, its share of its parent and its share of the total.",
      ],
    },
  ],
  use: [
    "You have a **hierarchy with sizes** and want to see the shape of the whole *and* drill into parts: sales by region, product and plan. Budget by division and project.",
    "There are two to four levels and each has a handful of children. That is where the rings stay readable.",
    "Parts of a whole is the story, and the nesting matters: what belongs to what is as important as how big it is.",
    "The audience will click. The drill-down centre makes this a tool for exploration, not only a picture.",
  ],
  avoid: [
    "You need to compare sizes precisely. Angles are harder to judge than lengths, and the same angle looks bigger on an outer ring. Use [Icicle](/charts/icicle) or a bar chart.",
    "The tree is deep (more than five levels) or very lopsided. The outer rings become tangles of slivers.",
    "The data is not nested. A sunburst of one level is a pie chart with extra steps.",
    "Values can be negative or zero in meaningful ways. Non-positive rows are dropped, so they cannot be shown.",
  ],
  read: [
    "Start in the centre: that is the whole, with its name and total printed in the hole.",
    "Read the first ring as an ordinary pie. Each slice is a top-level group. Its colour is the family colour for everything beneath it.",
    "Move outward. A slice's children sit directly outside it within its angle. Pale wedges are deeper levels.",
    "Click any wedge to make it the centre and give its children the full circle. Click the centre to go back up one level.",
    "Hover for value, share of parent and share of total.",
  ],
  story: {
    title: "The pie chart that wanted a sequel",
    paragraphs: [
      "Anneke worked at a chain of eleven garden centres in the Netherlands, in the finance department, which she described as being the department that gets invited to the Christmas party but not to the planning. Once a year she presented revenue to the board, and once a year the board asked her a follow-up that the slide could not answer.",
      "The slide was a pie of revenue by category: plants, tools, furniture, seasonal. The follow-up was always \"and what's in seasonal?\", and the answer was a second slide, and then a third for \"and the plants?\", and then the CEO asked to go back to the first one.",
      "Her task was to put the whole tree on a single page: category, then subcategory, then product line, with the sizes right and the structure obvious. Preferably in a form that would survive being projected onto a wall in a meeting room whose curtains did not close.",
      "She exported one row per product line with a path like `Plants/Perennials/Lavender` and the revenue, and gave them to `<Sunburst>` in bui-charts with the default separator. The centre read \"Total\" with the year's revenue, and the first ring looked like the pie the board was used to.",
      "The outer rings were the surprise. Under Seasonal sat a thick slice called Christmas trees, and under Plants a surprisingly wide wedge called Herbs, which, thanks to a supplier contract, were three times as profitable as anyone had assumed. She clicked into Seasonal during the meeting and the CEO's question was answered before he finished it.",
      "The board spent the hour looking at the chart rather than asking for the next one, which Anneke counts as a victory of format over human nature. Herbs got a bigger shelf. The Christmas trees got a polite round of applause, and a note that they were only profitable for eleven days.",
      "She still presents it every year. The curtains have since been replaced.",
    ],
  },
  pitfalls: [
    "**The path is the structure.** `Plants/Herbs` and `plants/herbs` are different nodes. The chart splits on the separator and trims spaces, but it does not change case.",
    "**Angle is the encoding, not area.** An outer-ring wedge with the same angle covers more area than an inner one, and looks bigger than it is.",
    "**Non-positive values vanish.** A row with a zero or negative value is dropped, and so is its path if nothing else shares it.",
    "**Wedges under about 0.002 radians are not drawn.** A tiny slice is not a bug. Zoom in by clicking its parent.",
    "**Order is first appearance.** Siblings are placed in the order the data introduces them, so sort your rows if you want them ranked by size.",
  ],
  related: [
    { slug: "icicle", why: "The same tree as rows of blocks. Widths are easier to compare than angles." },
    { slug: "mekko", why: "When you only have two levels and want area to carry the answer." },
    { slug: "waffle", why: "When you want a single level and need people to count units, not judge angles." },
  ],
  media: [
    { kind: "youtube", id: "sxyMRM2okx0", title: "Tell the story with an Excel SUNBURST Chart!", channel: "Exceleration Nation", note: "Shows how a sunburst reads hierarchies as concentric rings, built in Excel rather than this library." },
  ],
}
