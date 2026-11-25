import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One horizontal bar per task, positioned by start and end on a shared time axis.",
  intro: [
    "Every project plan eventually becomes a list of tasks with dates, and every list of tasks with dates eventually gets looked at by someone asking \"so is this going to be ready?\" A table of start and end dates cannot answer that. A Gantt chart can, because it lays the tasks on a single timeline and lets the reader see at a glance what runs when, what overlaps and where the gaps are.",
    "The form is named for [Henry Gantt](https://en.wikipedia.org/wiki/Henry_Gantt), the American engineer who popularised it in the 1910s for scheduling factory work, though a Polish engineer, [Karol Adamiecki](https://en.wikipedia.org/wiki/Karol_Adamiecki), had published a similar scheme earlier. Gantt charts went on to schedule shipyards, dams and, in due course, Mondays. They have survived because the idea is almost embarrassingly simple: time goes left to right, a task is a bar, and the bar's length is how long it takes.",
    "This one is the simple version on purpose. It draws the schedule you give it. It does not compute dependencies or critical paths, and it will not tell you the project is late. It will, however, make it very hard to claim that two tasks that overlap by six weeks are \"sequential\".",
  ],
  math: [
    {
      heading: "A bar is an interval on a shared axis",
      body: [
        "Each row of your data is a task with a name, a start $s_i$ and an end $e_i$, both numbers in the same unit as the x axis (by default a time axis, so epoch values). Every task becomes its own lane, drawn in the order of the data, top to bottom.",
        "The chart treats the bar as a half-open interval: task $i$ is \"on\" at every instant $t$ with $s_i \\le t < e_i$ and \"off\" otherwise. A task that ends at the instant another starts therefore touches it without overlapping. This is the same state-timeline machinery that underlies `<StateTimeline>`, with a lane per task and one state per lane.",
      ],
      tex: ["\\text{on}_i(t) = \\begin{cases} 1 & s_i \\le t < e_i \\\\ 0 & \\text{otherwise} \\end{cases}"],
    },
    {
      heading: "From dates to pixels",
      body: [
        "The x axis is a linear scale over the plot width $W$. Its minimum is the earliest finite start or end and its maximum is the latest, unless you set `x_axis` bounds yourself. The left edge and width of a bar are then linear in time. Non-finite starts and ends are ignored when the extent is computed.",
        "All the tasks are laid out on one page, so the lane height is the plot height over the number of tasks. The bar itself fills `row_height` of its lane (default 0.7), leaving the rest as padding. Fill opacity is 0.85 with no outline.",
      ],
      tex: [
        "x_i = \\frac{s_i - t_{\\min}}{t_{\\max} - t_{\\min}}\\,W, \\qquad \\ell_i = \\frac{e_i - s_i}{t_{\\max} - t_{\\min}}\\,W",
      ],
    },
    {
      heading: "Colour and labels",
      body: [
        "Colour follows a label: the `group` if you provide one, and the task name otherwise. The same label always gets the same colour, which is how a phase (design, build, test) can be shown by colour while every row still has its own name. The label is printed inside the bar according to `show_value`, if it fits.",
        "The legend lists the labels. Nothing is drawn between bars: there are no dependency arrows, no progress fill and no today line. If you need a marker for today, add one with the axis or an annotation in your own view.",
      ],
    },
  ],
  use: [
    "You want to show **what runs when** for a plan, a release schedule, a maintenance window or a hiring pipeline.",
    "Overlap matters. Two bars lined up on the same dates are the simplest way to say two things compete for the same people.",
    "Tasks fall into phases or owners and you want colour to say which. One `group` field gives you that.",
    "The audience is not technical. A Gantt chart needs no explanation, which cannot be said of much in a project office.",
  ],
  avoid: [
    "You have hundreds of tasks. One lane per task means one pixel per task. Group them into summaries or filter first.",
    "The point is the dependency structure. Bars alone do not show what blocks what. Use a [sankey](/charts/sankey) or a [node graph](/charts/node-graph) for flows and dependencies.",
    "You want to show states that change many times for the same thing, such as a server being up or down. [State timeline](/charts/state-timeline) is built for that: many segments per row.",
    "The times are only approximate. A Gantt chart looks precise, and a bar that ends on a Thursday suggests a Thursday that nobody has promised.",
  ],
  read: [
    "Read each bar's left edge as the start and its right edge as the end. The bar's length is the duration.",
    "Look down a vertical line at any date: every bar that crosses it is work in progress on that date.",
    "Look for the gaps and the stacks. A gap is slack, a stack of overlapping bars is a crunch.",
    "Colours mark the group or phase. Hover a bar for its name and time range.",
  ],
  story: {
    title: "The six weeks that were really nine",
    paragraphs: [
      "Hana was the programme manager at a mid-sized logistics firm called Northbound, and she had been asked to plan the move of its warehouse system to a new platform. The plan had eleven tasks, each with an owner, a start and an end. The plan lived in a spreadsheet, and the spreadsheet said the move would take six weeks.",
      "It was a good spreadsheet, tidy and colour coded. It had also been agreed by every owner, each of whom had looked only at their own row.",
      "Hana did not trust it, but she could not say why. A table of start and end dates hides one kind of fact very well: the facts that appear only when two rows are looked at together. She needed the tasks to be somewhere they could see each other.",
      "She fed the rows into `<Gantt>` from bui-charts: name, start, end and a group for each phase (data, integration, cutover). The chart took no configuration to speak of. It simply lined the eleven bars up against a shared calendar.",
      "The picture showed what the spreadsheet had not. The data migration and the integration tests were both assigned to the same three engineers, and in the third and fourth week the two bars sat on top of each other like a pair of very optimistic roofs. The integration tests also ended on the same day the cutover started, which would have been fine if integration tests ever ended on time.",
      "Hana added the overlap as an annotation on the slide and moved the cutover back by three weeks. The plan was now nine weeks, which the steering group accepted without complaint, since they had just watched the overlap with their own eyes. The move shipped in the ninth week, with one day of slack.",
      "Hana says the chart did not make the project any shorter. It just made the lie shorter-lived, which she counts as the most valuable thing a planning tool can do.",
    ],
  },
  pitfalls: [
    "**Same unit for start and end.** Both must be on the axis scale. If the axis is time, give both as epoch values. A duration in the end column will draw a very short bar at 1970.",
    "**Order is your order.** Tasks are drawn in the order of the data. Sort by start time (or phase) if you want the waterfall look.",
    "**End is exclusive.** A task that ends at midnight does not occupy that midnight. For day-grained plans, set the end to the day after the last working day.",
    "**No dependencies.** Bars do not know about each other. A task can start before the one it depends on finishes and the chart will not object.",
    "**Many tasks, one page.** Lanes shrink to fit all rows. Past a few dozen tasks, row labels become unreadable and you should summarise.",
  ],
  related: [
    { slug: "state-timeline", why: "When each row changes state many times, rather than having one bar." },
    { slug: "traces", why: "When the rows are nested spans of a single operation, with parent and child relationships." },
    { slug: "sankey", why: "When the interesting thing is the flow of effort or money between stages, not the dates." },
  ],
  media: [
    { kind: "youtube", id: "tIxIfLfTCF0", title: "What is a Gantt chart?  ...explained in under 3 minutes", channel: "Chris Croft", note: "Quick and clear explanation of how to read a Gantt chart." },
    { kind: "youtube", id: "PniUguqy7AY", title: "What Is a Gantt Chart - Easy Explanation", channel: "TeamGantt", note: "Explainer from a project-management software vendor." },
  ],
}
