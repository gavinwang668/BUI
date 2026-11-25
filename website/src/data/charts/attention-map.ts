import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of weights between queries and keys, one cell per pair, darker where the weight is higher.",
  intro: [
    "When a language model reads \"the animal didn't cross the street because it was too tired\", something inside it has to decide what \"it\" refers to. That decision is made by attention: for every word, the model computes how much to look at every other word. Those numbers form a table, one row per word doing the looking and one column per word being looked at. An attention map is that table, coloured.",
    "The picture is simple and, since the transformer architecture arrived in 2017, it has become the first thing people draw when they want to look inside a model. Row by row, you can see which tokens a token leaned on. A bright diagonal means \"I mostly look at myself\". A bright column means \"everybody looks at this one word\", which is frequently the first token or a full stop, to the endless amusement of people who hoped it would be profound.",
    "The chart is useful beyond language models: any matrix of non-negative weights between two lists of labels fits. Translation alignments, co-occurrence tables, routing weights in a mixture of experts. If it is rows of things that look at columns of things, this is how you see it.",
  ],
  math: [
    {
      heading: "Where the numbers come from",
      body: [
        "The chart does not compute attention. It draws the numbers you give it. In a standard [transformer](https://arxiv.org/abs/1706.03762) layer, a token's query $q_i$ is compared with every key $k_j$ by a scaled dot product, and a softmax turns each row of scores into weights that are positive and sum to 1. For a head with key dimension $d_k$ that gives the matrix $A$ below. The chart's `weights` parameter is that matrix, flattened row by row. Row $i$ (a query) is drawn at the top when it is first, as in a matrix. Column $j$ (a key) is along the bottom.",
      ],
      tex: [
        "A_{ij} = \\frac{\\exp\\bigl(q_i \\cdot k_j / \\sqrt{d_k}\\bigr)}{\\sum_{j'} \\exp\\bigl(q_i \\cdot k_{j'} / \\sqrt{d_k}\\bigr)}, \\qquad \\sum_j A_{ij} = 1",
      ],
    },
    {
      heading: "What gets drawn",
      body: [
        "With $L_q$ queries and $L_k$ tokens, `weights` must hold $L_q \\times L_k$ values: the cell at row $i$ and column $j$ is entry $i L_k + j$. A missing weight (the list is too short) is left empty rather than drawn as zero. If you leave out `queries` the rows use the same labels as `tokens`, which is the self-attention case.",
        "Each cell is a unit square centered at integer coordinates, separated from its neighbours by a 1 pixel gap, and coloured by the weight through a sequential scheme (`Blues` by default. Pass `scheme` for another). The axes are labelled *Query* and *Key*.",
      ],
    },
    {
      heading: "Printing the values",
      body: [
        "Weights are written in their cells with two decimals when there are 12 or fewer columns, because beyond that the cells are too small for text. Pass `show_values={true}` to force them on or `false` to force them off.",
        "A useful sanity check: every row of a softmax attention map sums to 1, so the sum of a row's printed numbers should be close to 1 up to rounding. If it is not, you are drawing pre-softmax scores or a column-wise normalisation, and the picture says something different.",
      ],
      tex: ["\\sum_{j=1}^{L_k} A_{ij} \\approx 1 \\quad \\text{for every row } i"],
    },
  ],
  use: [
    "You want to see which input tokens a model attended to while producing each output or hidden state.",
    "You have a matrix of relationships between two lists of labelled items, for instance source and target words in a translation alignment.",
    "You are debugging a single head or a short sequence, up to a few dozen tokens, where each cell is still readable.",
    "You teach, or write, about transformers and want a picture of attention that is honest about being a table.",
  ],
  avoid: [
    "The sequence has hundreds of tokens. Cells shrink to dots and the labels overlap. Average over heads, pool, or pick a window.",
    "You need to say what the model *relies on*. Attention weights are not an explanation of the output, and [the literature has argued about this](https://arxiv.org/abs/1902.10186) for years. Treat the map as a hint, not as evidence.",
    "The rows are not comparable to each other. A matrix of raw scores with different scales per row will light up the largest row and hide the rest. Normalise first.",
    "The matrix has negative entries that matter. The colour scheme is sequential, so sign is not shown. Use a diverging scheme in a [heatmap](/charts/heatmap).",
  ],
  read: [
    "Rows are queries, columns are keys. Read along a row to see where that token looked.",
    "A dark diagonal means tokens attend to themselves. A dark band just below it means they attend to the previous token.",
    "A dark column means one key is a magnet for everyone else. Check whether it is the first token or a delimiter before you get excited.",
    "In a causal model the cells above the diagonal are empty or zero, because tokens cannot look at the future. A triangle is the correct picture.",
    "Hover a cell for its query, key and weight.",
  ],
  story: {
    title: "The pronoun problem",
    paragraphs: [
      "Joaquín taught the first week of a machine-learning bootcamp, which meant he had forty students, one projector and the unenviable task of explaining attention in a morning. His previous attempt had involved a matrix equation and a lot of silence.",
      "This time he wanted something with a punchline. He took a small pretrained model, fed it the sentence \"The trophy didn't fit in the suitcase because it was too big\", extracted the attention weights from one head in the middle layers, and prepared to show that the model knew what \"it\" meant.",
      "The first picture he drew was a table of numbers with 11 columns and 11 rows. It was true. It was not what anyone in the room could read, and a student in the second row asked whether the red cells meant a bug.",
      "He needed the same matrix as a picture a nervous beginner could scan: tokens down the side, tokens along the bottom, darker where the weight is higher, and each weight in its cell for the one person who wants to check. That is a very specific shape, and he had it already.",
      "He passed the token list and the flat weights to an `<AttentionMap>` in bui-charts, with `tokens` and `weights` and nothing else. The \"it\" row had its darkest cell under \"trophy\", at 0.41, and a weaker one under \"suitcase\", at 0.22. The room got it immediately. A hand went up: \"so it picked the right one?\"",
      "He said yes, and then spent ten minutes on the more interesting part. The same head, on the sentence with \"small\" instead of \"big\", did not flip. Attention was one ingredient, not the whole decision. The students filled the board with questions, and the survey at the end of the week gave the session an average of 4.7 out of 5, the best of the course, ahead of the free lunch.",
      "Joaquín keeps the first table of numbers in his slides, as a warning. The picture is the same data, he tells the next class, just better dressed.",
    ],
  },
  pitfalls: [
    "**Attention is not explanation.** A high weight says the model mixed that token's value in strongly, not that it caused the answer. Be careful what you claim from a map.",
    "**Which head, which layer?** Models have dozens of heads, and each map is one of them. Averaging them is common and hides the sharp heads. Say which one you are showing.",
    "**Rows and columns swapped.** Weights go row by row, one row per query. If your matrix is transposed the picture still draws, but the story it tells is the reverse.",
    "**Long sequences with values on.** Beyond 12 columns the numbers are hidden for a reason. Forcing them on with `show_values` gives you an unreadable grid.",
  ],
  related: [
    { slug: "heatmap", why: "When the grid is values over time and value buckets, or you need a diverging colour scale." },
    { slug: "sankey-matrix", why: "When you also want to see the flows between rows and columns, with exact values." },
    { slug: "chord-diagram", why: "When the matrix is a small set of relationships between the same items and a circle reads better." },
  ],
  media: [
    { kind: "youtube", id: "eMlx5fFNoYc", title: "Attention in transformers, step-by-step | Deep Learning Chapter 6", channel: "3Blue1Brown", note: "The clearest walkthrough of what the query, key and softmax steps compute, and so of what each cell in this map means." },
  ],
}
