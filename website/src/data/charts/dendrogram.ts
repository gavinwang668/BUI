import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A tree that shows which rows are most alike, built by repeatedly merging the two closest groups.",
  intro: [
    "Clustering has an awkward first question: how many clusters? K-means makes you answer before it starts, which is a bit like being asked to choose the number of chapters before writing the book. [Hierarchical clustering](https://en.wikipedia.org/wiki/Hierarchical_clustering) sidesteps the problem by refusing to commit. It starts with every row on its own, merges the two most similar, then the next two, and keeps going until only one blob remains. The record of those merges is the dendrogram.",
    "The word is Greek for \"tree drawing\", and the picture earns it. Leaves along the bottom are your rows. Every U-shaped join is one merge, and the **height** of the join is how far apart the two things being merged were. Tight pairs join low. Odd rows hold out and join high up, near the trunk, like the cousin who arrives at the party after dessert.",
    "The best feature of a dendrogram is that you pick the cut afterwards. Slice it low and you get many small, very similar groups. Slice it high and you get a few big ones. The tree has the answer to every value of *k* at once, and the picture helps you see where the natural joints are: a long vertical gap before a merge means two groups that really are distinct.",
  ],
  math: [
    {
      heading: "Distances between rows",
      body: [
        "Each row gives you a vector of numbers $x_i \\in \\mathbb{R}^p$ through the `features` function. Rows are compared by plain Euclidean distance. If a feature is missing or not finite for a pair, that coordinate is skipped for that pair, so a row with a hole is compared on what it has instead of dropping out. Nothing is rescaled: a feature measured in thousands will outvote one measured in tenths, so put features on similar scales yourself.",
      ],
      tex: [
        "d(x_i, x_j) = \\sqrt{\\sum_{f=1}^{p} (x_{if} - x_{jf})^2}",
      ],
    },
    {
      heading: "Merging: linkage",
      body: [
        "The algorithm starts with $n$ singleton clusters and, at every step, finds the pair of clusters $A$ and $B$ with the smallest gap and merges them into $A \\cup B$. What differs between linkages is how the gap from the new cluster to some other cluster $C$ is computed from the gaps that already exist. All four use the Lance-Williams update, so each step is a cheap formula on the old distances rather than a recomputation from the points.",
        "**Single** keeps the minimum of the two old gaps and tends to chain. **Complete** keeps the maximum and keeps clusters tight. **Average** (the default) takes the size-weighted mean. **Ward** works on *squared* distances and minimises the growth in within-cluster variance. $n_A$ is the number of rows in $A$.",
      ],
      tex: [
        "d_{\\text{single}} = \\min\\bigl(d_{AC}, d_{BC}\\bigr), \\qquad d_{\\text{complete}} = \\max\\bigl(d_{AC}, d_{BC}\\bigr)",
        "d_{\\text{average}} = \\frac{n_A\\, d_{AC} + n_B\\, d_{BC}}{n_A + n_B}",
        "d^2_{\\text{Ward}} = \\frac{(n_A + n_C)\\, d^2_{AC} + (n_B + n_C)\\, d^2_{BC} - n_C\\, d^2_{AB}}{n_A + n_B + n_C}",
      ],
    },
    {
      heading: "Drawing the tree",
      body: [
        "The merge sequence is a binary tree with $n$ leaves and $n - 1$ internal nodes. Leaves are ordered by a depth-first walk from the root, left child before right, and leaf $s$ in that order sits at position $s$. Each internal node sits halfway between its children and at the height of its merge. For Ward the stored height is the square root of the squared gap, so it is in the same units as the data. A merge is drawn as a bracket: down from the node to each child's position.",
        "The order of the walk is whatever the merge order gives. It is not re-arranged to put the most similar leaves side by side, so two similar rows can end up separated by an unrelated subtree. Heights, not left-right positions, carry the similarity.",
        "Setting `clusters` to $k$ cuts the tree as if you had asked for $k$ groups: the last $k - 1$ merges (the highest ones) are left neutral and everything below them is coloured by the group it belongs to. With $k = 1$ (the default) there is nothing to cut and the whole tree is one colour. The work is cubic in the number of rows, so a few hundred is fine and ten thousand is not.",
      ],
    },
  ],
  use: [
    "You want to **group rows by similarity** without choosing the number of groups first, and you want to see how the groups nest.",
    "The rows are few enough to label (up to a few hundred): countries, products, genes, patients, teams.",
    "You are exploring. A dendrogram is one of the quickest ways to find out whether your data has structure, or only has rows.",
    "You want to compare how different linkages group the same data. Switching one prop changes the tree and the story.",
  ],
  avoid: [
    "The features are on wildly different scales and you have not normalised them. The tree will mostly reflect the biggest feature.",
    "You have thousands of rows. The labels cannot fit and the cubic cost starts to hurt. Cluster first with something else, or sample.",
    "You need to say how *confident* a grouping is. A dendrogram is a description of the merge order, not a hypothesis test.",
    "The rows have a natural order, like time. Use a line chart. Clustering them will scramble what you already know.",
  ],
  read: [
    "Find the leaves along the bottom (or the left, with `horizontal`): each one is a row. Read the labels first.",
    "Trace two leaves up to where they meet. The height of that join is the distance at which they became one group. Lower means more alike.",
    "Look for long vertical stretches with no merge. A long gap is a natural cut: the groups below it are well separated from each other.",
    "Use `clusters` to colour the tree for a chosen cut. Links above the cut stay neutral, so the colours show you the groups at a glance.",
    "Do not read left-to-right proximity as similarity. Only the joins carry meaning.",
  ],
  story: {
    title: "Seven stores that were secretly two",
    paragraphs: [
      "Mirela was the data analyst for a coffee roaster with seven small shops and one very strong opinion about oat milk. The operations director wanted one staffing plan per shop. The finance director wanted one plan for the whole company. The two of them had been having this conversation since March, and it had acquired the quality of a family tradition.",
      "Mirela suspected the truth was in between, but suspicion is not a slide. She had hourly sales for each shop averaged into twelve numbers, one for each two-hour block over a typical day, and a growing feeling that some of the shops behaved more like each other than their addresses implied.",
      "The task was to find out how many different kinds of shop there really were, without anyone having to pick the number in advance, because whichever number she picked would be the one the room argued about.",
      "She normalised each shop's twelve numbers into shares of that shop's own day, so a busy shop and a quiet one with the same rhythm would look alike, and fed them to `<Dendrogram>` from bui-charts with average linkage. Seven labelled leaves, six brackets, and one very obvious thing.",
      "Five of the shops merged at tiny heights, almost on top of each other: the morning rush, a dip, a small afternoon bump. The other two joined each other low and everyone else very high. Those two were the shops by the university, which did not have a morning rush so much as an afternoon-into-evening one, with a long study-session tail.",
      "She set `clusters` to two and the colours made it undeniable. The company ran two staffing patterns, not seven and not one. The operations director got a plan for the two campus shops, the finance director got a template for the other five, and the extra hours at the university shops cut walk-aways during the evening queue by about a fifth.",
      "Both directors claimed to have been right all along. Mirela has stopped correcting them, and keeps the tree on her wall.",
    ],
  },
  pitfalls: [
    "**Scale decides everything.** Euclidean distance on raw features gives the biggest unit all the votes. Scale to 0-10 or z-scores first.",
    "**Linkage changes the tree.** Single chains, complete keeps groups tight, average is a compromise, Ward makes compact even groups. If a conclusion survives all four, you can trust it more.",
    "**Left-right closeness means nothing.** Neighbouring leaves may be unrelated, because only the heights of the joins carry distance.",
    "**Cutting is your decision.** `clusters` colours a cut. It does not choose a good one. Look for the longest vertical gap before picking.",
    "**Cost grows with the cube of the row count.** A few hundred rows is comfortable. Beyond that, reduce the data first.",
  ],
  related: [
    { slug: "heatmap", why: "When you want to show the values behind the clusters, not only the grouping." },
    { slug: "icicle", why: "When the hierarchy is already known (a budget, a disk) and does not need to be discovered." },
    { slug: "parallel-coordinates", why: "When you want to see which features make the groups differ." },
  ],
  media: [
    { kind: "youtube", id: "ijUMKMC4f9I", title: "Flat and Hierarchical Clustering | The Dendrogram Explained", channel: "365 Data Science", note: "Explains how to read a dendrogram and cut it into clusters." },
    { kind: "youtube", id: "8QCBl-xdeZI", title: "Hierarchical Cluster Analysis [Simply explained]", channel: "numiqo", note: "Walks through how hierarchical clustering produces the tree." },
  ],
}
