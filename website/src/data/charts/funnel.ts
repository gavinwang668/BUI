import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Centered bars, one per stage, so you can see how many are left at every step.",
  intro: [
    "Every process that involves people eventually loses some of them. They close the tab, forget the cart, miss the email, or realise halfway through the signup form that they were only curious. The question is never *whether* people drop out. It is *where*, and whether that stage is leaking more than its neighbours.",
    "The funnel chart answers it with the oldest trick in the sales book. The idea comes from the [sales funnel](https://en.wikipedia.org/wiki/Purchase_funnel), a metaphor sales and marketing people have been drawing for about a century: lots of prospects enter at the top, a few come out of the bottom, and everybody in the middle is told to stay positive. The chart turns the metaphor into data. One bar per stage, each as long as the number of people still in the process, all centered on the same line.",
    "Centering is the whole design. Line the same bars up on the left and you have a bar chart that happens to be sorted. Center them and the eye sees a shape, and a sudden pinch in that shape is a stage that deserves a meeting. Nobody has ever solved a drop-off by staring at a bar chart, but plenty of people have solved it by staring at a funnel and saying \"well, that's obviously wrong\".",
  ],
  math: [
    {
      heading: "What the chart takes",
      body: [
        "You give `<Funnel>` one row per stage, in the order the stages happen, with a name and a value. Write the values as $v_1, \\dots, v_n$. The chart does not sort them and does not insist that they decrease: a stage that is larger than the one before it simply draws longer. Non-finite values are skipped when finding the widest stage.",
        "The only number the chart derives is the widest stage, $v_{\\max}$. Everything is drawn against that.",
      ],
      tex: ["v_{\\max} = \\max_{i}\\, v_i"],
    },
    {
      heading: "Centering",
      body: [
        "Each bar starts at an offset and extends by its value. The offset is half of whatever the bar is missing relative to the widest one, which puts the midpoint of every bar at the same place, $v_{\\max}/2$.",
        "The value axis is fixed to start at $0$, hidden, and has no grid. That is deliberate: in a funnel the bar lengths are the message, and the exact numbers are written on the bars (`show_values`, on by default) and in the tooltip.",
      ],
      tex: [
        "o_i = \\frac{v_{\\max} - v_i}{2}, \\qquad \\text{bar } i \\text{ spans } [\\,o_i,\\; o_i + v_i\\,]",
      ],
    },
    {
      heading: "Stages and colour",
      body: [
        "The stages sit on the category axis, one slot each, and every bar fills 85% of its slot, so the gap between bars is $0.15$ of a slot. By default each stage takes the next colour of the theme's series palette. Pass `color` and every stage uses that one colour instead. The legend is hidden because the axis already names the stages.",
        "With `horizontal` on (the default) the stages go down the page and the bars grow sideways. Turn it off and they stand up, which suits a short funnel with long stage names much worse than you would hope.",
      ],
    },
  ],
  use: [
    "You have an ordered process and a count at each step: visitors, signups, activations, payers. The shape is the point.",
    "You want a team to agree on *where* the leak is before arguing about why. A funnel makes the biggest step-to-step drop visible without any arithmetic.",
    "You are presenting to people who do not read charts for a living. Everyone already has a funnel in their head, and this one matches it.",
    "You have five to eight stages. That is roughly where the chart stays readable and still has something to say.",
  ],
  avoid: [
    "The stages are not sequential. Categories that merely sit side by side are better served by a [lollipop](/charts/lollipop) or an ordinary bar chart.",
    "You care about the conversion rate between steps more than the counts. Print the rates in a table next to the chart, or use a [Sankey funnel](/charts/sankey-funnel) where drop-offs get their own flows.",
    "People can re-enter or skip stages. A funnel assumes one path, and real customers are rarely that tidy.",
    "You have dozens of stages. Past about ten the bars become a staircase and the pinch you wanted to find hides in it.",
  ],
  read: [
    "Start at the top bar: that is everyone who entered. Its length is the scale for everything below it.",
    "Walk down and watch for the step where the bar shrinks the most relative to the one above. Compare the *ratio* of neighbours, not the absolute drop. A loss of 40,000 from a bar of 100,000 is a different story from a loss of 40,000 from 1,000,000.",
    "The bars are centered, so a lopsided stage is not a bug in the chart. If a stage is wider than the one before it, the data says so: check your definition of the stage.",
    "Hover a bar for its stage name and value. The numbers printed on the bars let you skip the hover for the obvious cases.",
  ],
  story: {
    title: "Forty-one percent of Tuesday",
    paragraphs: [
      "Ines was the only analyst at a six-person company that sold subscription socks. The founders called it a \"sock-as-a-service platform\" in meetings and \"socks\" everywhere else. Revenue was fine. What kept Ines awake was that nobody could say why it was not better.",
      "Every Monday the team looked at a table: 48,200 visitors, 11,900 who viewed a product, 3,100 who added something to the cart, 1,240 who reached checkout, 910 who paid. The table was correct and totally inert. Each person read it, nodded, and went back to opinions they already had.",
      "The marketing lead was sure the problem was traffic quality. The designer was sure it was the product page. The founder, who wrote the checkout himself, was sure it was fine, and said so with the calm of a man who had not recently tried to use it.",
      "Ines wanted the argument to start from the same picture. Not a better table, a shape: five stages, one line down the middle, and everyone able to see at once which step was narrower than it should be.",
      "She fed the five rows into a `<Funnel>` from bui-charts, name and value, nothing else, and put it on the Monday screen. The first three bars looked like a normal slope. Then the cart bar and the checkout bar sat nearly side by side, and the paid bar dropped to three quarters of the checkout bar, a funnel with a neck where a neck should not be.",
      "Nobody had to be convinced. The cart-to-checkout step kept 40% of people, and the designer pointed out that the shipping cost only appeared on that page. They moved the estimate into the cart. Four weeks later checkout reached 1,780 people a month instead of 1,240, and paid orders went from 910 to 1,260, a 38% jump, with no new traffic and no new socks.",
      "The founder admitted the checkout page was not fine. He did it in a quiet voice, looking at the screen, which is what a funnel is for: it lets people disagree with a picture instead of with each other.",
    ],
  },
  pitfalls: [
    "**Reading width as time or cost.** Only the length matters. The funnel shape is made of a stack of bars, so do not treat the area or the slope between stages as data.",
    "**Stages that are not nested.** If someone can reach stage 4 without passing stage 3, the later bar can be longer than the earlier one. The chart will draw it honestly, and it will look like a mistake.",
    "**Mixing units between stages.** If the top is *sessions* and the bottom is *people*, the pinch you see is partly an artefact. Count the same thing at every step.",
    "**Colour as decoration.** Each stage gets its own colour by default. The colours do not mean anything. If you want one message, pass `color` and let the lengths speak.",
  ],
  related: [
    { slug: "sankey-funnel", why: "When you need to see where the people who left actually went, not just how many remain." },
    { slug: "waterfall", why: "When you want the *losses* at each step as bars of their own, adding up to the end value." },
    { slug: "lollipop", why: "When the items are not in a sequence and you only want to compare sizes." },
  ],
  media: [
    { kind: "youtube", id: "MSw9GX7r8qE", title: "Summarizing a process using a funnel chart", channel: "Curt Frye", note: "A calm, short walkthrough of what a funnel chart is for: stages, counts, and where the leak is." },
  ],
}
