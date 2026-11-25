import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A fishbone diagram: one problem at the head, its possible causes sorted onto bones.",
  intro: [
    "Something is wrong. The bores are oversized, the checkout is slow, the soup is cold. Somebody calls a meeting, and within four minutes there are nineteen theories on the whiteboard, all written in different handwriting, none of them filed under anything. The Ishikawa diagram exists to stop that meeting from becoming a weather system.",
    "It is named after [Kaoru Ishikawa](https://en.wikipedia.org/wiki/Kaoru_Ishikawa), the Japanese quality-management writer who popularised it in the 1960s as one of the basic tools for factory-floor problem solving. It is also called a fishbone diagram, because it looks like the skeleton of a fish that has had a hard week, and a cause-and-effect diagram, by people who want you to know it is serious. The head is the effect you are chasing. The spine runs into it. Each bone is a *category* of cause, and the small ribs hanging off a bone are the actual suspects.",
    "Notice what it does not do. It does not tell you which cause is the real one, and it does not measure anything. It is a way of making a team argue in an orderly fashion. That sounds modest until you have sat through the disorderly version.",
  ],
  math: [
    {
      heading: "There is no statistic, only a grouping",
      body: [
        "An Ishikawa diagram is the one chart in this family where the data is not numeric. Each row is a pair of strings: a category $c$ and a cause $k$. The chart groups the rows by category, keeping the order in which categories first appear, and keeps the causes inside each category in the order they appear in your data.",
        "If the categories are $C_1, \\dots, C_n$ and category $C_i$ has $m_i$ causes, the picture is a set of ordered lists. Nothing is counted or weighted. A bone with seven causes is not seven times as important as a bone with one; it is just a bone someone had more to say about.",
      ],
      tex: ["C_i = (k_{i1}, k_{i2}, \\dots, k_{i m_i}), \\qquad i = 1, \\dots, n"],
    },
    {
      heading: "Where the bones go",
      body: [
        "The spine sits at half the chart height, running from a small left margin to just before the head. The head is as wide as its text plus a margin, clamped between 80 pixels and 30% of the chart width.",
        "The first $\\lceil n/2 \\rceil$ categories go above the spine and the rest below, so bones alternate by halves, not by turns. With $s = \\lceil n/2 \\rceil$ slots along the spine and a usable spine length $L$, slot $j$ has its foot at a spacing of $g = (L - 20)/s$ from the previous one. Every bone leans back from its foot by $0.55$ of its reach, and the reach is the distance from the spine to near the chart edge, never less than 20 pixels.",
      ],
      tex: ["g = \\frac{L - 20}{\\lceil n/2 \\rceil}, \\qquad \\text{reach} = \\max(\\tfrac{H}{2} - 28,\\; 20)"],
    },
    {
      heading: "Where the ribs go",
      body: [
        "A bone with $m$ causes puts rib $r$ at fraction $t_r = r/(m+1)$ along the bone, measured from the tip, so the ribs are evenly spaced and never touch either end. Each rib is a short horizontal line to the left of the bone. Its length is the text width plus 8 pixels, capped at 90% of the slot gap (and never below 40 pixels).",
        "If the text does not fit, it is cut and an ellipsis is added. Hovering a bone shows the full list of causes in a note, which is what the ellipsis is promising you.",
      ],
      tex: ["t_r = \\frac{r}{m + 1}, \\qquad r = 1, \\dots, m"],
    },
  ],
  use: [
    "You are brainstorming causes of one specific problem with a team, and you want the suggestions sorted as they arrive rather than after.",
    "You need a shared picture for a root-cause review or a post-mortem, where the categories (people, process, equipment, materials, environment, measurement) remind everybody what they have not yet thought about.",
    "You want a record of what was *considered*, not just what was blamed. The empty bones are information too.",
  ],
  avoid: [
    "You have data on how often each cause occurs. Then you want a [Pareto chart](/charts/pareto), which ranks by count and tells you where to start.",
    "The problem is not one problem. A fishbone with a head that says \"things are bad\" collects every complaint in the building and proves nothing.",
    "You need to show causes feeding other causes. The ribs are one level deep. Chains and loops want a [node graph](/charts/node-graph) or a tree.",
    "You have more than about eight categories. Bones crowd, labels shorten, and the diagram starts to look like a very ambitious bicycle rack.",
  ],
  read: [
    "Start at the head: that is the effect, the thing everybody agreed is wrong.",
    "Read each bone's name as a question: could something about *this* have caused it? Then read its ribs as the specific answers people proposed.",
    "Count nothing. Length and number of ribs reflect how much was said, not how likely the cause is.",
    "Hover a bone to see all of its causes in full, including those the chart had to abbreviate.",
    "Mark the two or three causes you will test first somewhere else, with evidence. The diagram only lists candidates.",
  ],
  story: {
    title: "Forty-one theories and a Tuesday",
    paragraphs: [
      "Marta ran quality at a small foundry that cast pump housings, and in March the housings began coming back from the machine shop with bores 0.08 mm too big. Not every housing. About one in nine, which is the worst possible rate: often enough to cost real money, rarely enough that everyone believes it is somebody else's batch.",
      "She called a meeting. By lunch the whiteboard held forty-one theories, including worn tooling, a new apprentice, humidity, a supplier's alloy, the second shift, and one person's firm belief that the building was haunted. The photograph she took of it was, she said later, the single least useful document the company had ever produced.",
      "What the board lacked was shape. Seven people had said \"tooling\" in seven different ways, and nobody could see that nothing at all had been said about measurement, which is where bores get their numbers.",
      "She exported the list to a spreadsheet, tagged every suggestion with one of six categories, and drew it with `<Ishikawa>` from bui-charts, effect \"Oversized bores\", category and cause read straight off the two columns. The result had six bones, and the Measurement bone had exactly one rib on it, which said \"gauge\".",
      "That was the bone the room stared at. Somebody asked when the bore gauge had last been calibrated. Nobody knew. The sticker said 2022, and the sticker had been written on with a marker that had since given up.",
      "The gauge was recalibrated on Tuesday and read 0.07 mm low, which made every borderline bore look fine when it was not. Scrap on the housings fell from about eleven per cent to under two over the next four weeks. The haunting was never formally ruled out, but it dropped off the list.",
      "A fishbone cannot find the cause. It can only show you which bone has been left suspiciously bare.",
    ],
  },
  pitfalls: [
    "**A vague head.** \"Low quality\" produces causes for everything. State the effect as a measurable thing, like \"bore over tolerance\", so each rib can be tested.",
    "**Treating ribs as evidence.** A rib is a hypothesis. Nothing in the diagram records whether anyone checked it.",
    "**Uneven categories.** If one bone has ten ribs and another has none, the team is describing where it has opinions, not where the problem lives. Look at the empty bones first.",
    "**Order matters.** Categories appear in the order they first show up in your rows, and the first half go above the spine. Sort your data if you want the six Ms in a fixed layout.",
    "**Long rib text.** Ribs are cut to fit and end in an ellipsis. Keep causes to a few words and put the detail in the hover.",
  ],
  related: [
    { slug: "pareto", why: "Once you have counts for each cause, this ranks them and shows the vital few." },
    { slug: "control-chart", why: "To check whether the effect you are chasing is a real shift in the process or ordinary noise." },
    { slug: "node-graph", why: "When causes cause other causes and a single level of ribs cannot hold the story." },
  ],
  media: [
    {kind:"youtube",id:"I0c6Gd26Fxw",title:"Fishbone (Cause & Effect or Ishikawa Diagram)",channel:"GEM Learning",note:"Short explanation of how to build and read an Ishikawa cause-and-effect diagram."},
  ],
}
