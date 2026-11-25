import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Stacks of letters whose height says how conserved each position of a set of aligned sequences is.",
  intro: [
    "Imagine two hundred strings of DNA, each twenty letters long, all of which a protein happily binds to. You line them up and stare. Position 7 is almost always a G. Position 12 is a coin toss between A and T. Position 3 is whatever. A consensus sequence would say GATTACA-something and throw away everything you just noticed about the confidence at each spot.",
    "A [sequence logo](https://en.wikipedia.org/wiki/Sequence_logo) keeps that confidence. At each position it draws a stack of the letters that occur there, each letter as tall as its frequency times the information the column carries, with the most common letter on top. The whole stack's height is a number in bits: how much the position tells you beyond random chance. Tall stacks are the places where evolution refused to compromise. Short, messy ones are the places it did not care. Thomas Schneider and Mike Stephens introduced the format in 1990 and it has appeared on the front of a remarkable number of papers since.",
    "The logo is a bar chart that has swallowed its own labels. The letters *are* the bars, and the only thing you need to know to read it is that tall means \"this position matters\".",
  ],
  math: [
    {
      heading: "Counting",
      body: [
        "Take $N$ aligned sequences, uppercased, over an alphabet of size $s$: $4$ for DNA and RNA, $20$ for protein. By default the alphabet is detected: DNA if every letter is in ACGT, RNA if every letter is in ACGU, protein otherwise. At each position $j$ the chart counts how often each letter occurs. Characters outside the alphabet, such as gap dashes, are ignored at that position, so $n_j$ is the number of valid letters there, not necessarily $N$. A position with no valid letters is left empty.",
        "The width of the chart is the length of the longest sequence. Shorter sequences simply contribute nothing at the positions they lack.",
      ],
      tex: [
        "p_{b j} = \\frac{n_{b j}}{n_j}, \\qquad n_j = \\sum_{b} n_{b j}",
      ],
    },
    {
      heading: "Information content",
      body: [
        "The uncertainty of a column is its [Shannon entropy](https://en.wikipedia.org/wiki/Entropy_(information_theory)) $H_j$ in bits. A column where all letters are equally likely has the maximum entropy $\\log_2 s$ ($2$ bits for DNA, about $4.32$ for protein). A column with one letter has zero. The information at a position is how far it sits below the maximum.",
        "A small sample lies in a particular way: with few sequences, a column can look conserved by chance. The chart applies the small-sample correction $e_j$ from Schneider's original paper, on by default (`correction`), and clamps the result at zero. This is why a logo built from five sequences looks short and one from five hundred looks tall at the same conserved positions. The comparison is against a uniform background. There is no adjustment for the base composition of the genome.",
      ],
      tex: [
        "H_j = -\\sum_{b} p_{b j}\\,\\log_2 p_{b j}, \\qquad e_j = \\frac{s - 1}{2 \\ln 2 \\cdot n_j}",
        "R_j = \\max\\bigl(0,\\; \\log_2 s - H_j - e_j\\bigr)",
      ],
    },
    {
      heading: "From bits to letters",
      body: [
        "Each letter's height in a stack is its share of the column times the column's information. The stack always sums to $R_j$, and the tallest letters sit on top, with the smallest at the baseline (they are sorted by height, ascending, from the bottom up). The vertical axis runs from $0$ to $\\log_2 s$, with a tick at each whole bit.",
        "Letters are drawn as real glyph shapes for A, C, G, T and U, scaled to fill their box: a letter as wide as its column and as tall as its height. Letters under half a pixel tall are skipped. With `units` set to probability instead of bits, heights are just $p_{bj}$ and the axis runs from $0$ to $1$, which shows frequency instead of information.",
      ],
      tex: [
        "h_{b j} = p_{b j}\\,R_j, \\qquad \\sum_{b} h_{b j} = R_j",
      ],
    },
  ],
  use: [
    "You have a set of **aligned sequences** (binding sites, motifs, a protein family) and want to show which positions are conserved and what the variants are.",
    "A consensus string would hide the confidence. The logo shows it, and a position that is 60/40 does not get flattened to one letter.",
    "You want to see at a glance which positions matter: tall stacks are constrained, short ones are free.",
    "You are comparing motifs from different experiments. Put logos in the same units and the same scale and read the shapes.",
  ],
  avoid: [
    "The sequences are not aligned. A logo assumes that column $j$ means the same thing in every row. Unaligned sequences produce confident nonsense.",
    "You have only a handful of sequences. The small-sample correction will flatten the logo, and any single mutation looks like a trend. Show the raw alignment instead.",
    "You need to show statistical significance. A logo is a summary, not a test.",
    "The sequences are very long (hundreds of positions). Letters shrink to nothing. Break the sequence into windows.",
  ],
  read: [
    "Read the vertical axis: it is in bits and its top is the most any position can carry (2 bits for DNA, about 4.3 for protein).",
    "Look at stack height. A tall stack is a conserved position. A flat one means anything goes.",
    "Look inside a stack. The letter on top is the most common one, and a stack of two letters of roughly equal height is a position that can be either.",
    "Use the position numbers along the bottom, which start at `start` (default 1) and can be set to match a gene or a motif numbering.",
    "Hover a letter to see its frequency and its height in bits.",
  ],
  story: {
    title: "The consensus that was only 60 percent sure",
    paragraphs: [
      "Ayesha was a second-year PhD student in a lab that studied how a soil bacterium turned on its antibiotic genes. She had 84 short DNA sequences that a certain regulatory protein bound, pulled from the genome after a long and expensive experiment. Her advisor wanted a figure, and the figure had to explain, in half a column, what the protein looked for.",
      "The honest version of that figure is a consensus, and she wrote it down: TGTGA, a gap, then TCACA. The advisor looked at it and asked how sure she was about each letter. She said \"fairly\", which is the kind of answer that gets a student a longer meeting.",
      "She needed to show the confidence at each of the 20 positions, along with the alternatives that really occurred, in one panel and without a table of counts. Anything less and a reviewer would ask for the table anyway.",
      "She put the sequences into `<SequenceLogo>` from bui-charts, set `start` to -10 so the numbers would match the transcription start, and left the alphabet on auto. The stacks came up like a skyline: a tall run of five letters at the left, a flat marshland in the middle, and five tall letters on the right, nearly a mirror of the first five.",
      "She had expected the left side to be strong. What she had not expected was that position 4, which she had written as a confident G in the consensus, was a stack of G on top and a thick slab of A beneath, almost 40% of the sequences. Her consensus had been a plausible lie. The logo was the first thing that made the protein look like a real, slightly sloppy protein.",
      "The figure went into the paper as the first panel. A reviewer said it was \"unusually informative\", which Ayesha has framed. Her advisor stopped asking how sure she was and started asking what the A did.",
      "Two years later, someone mutated it, and the answer was in the logo all along.",
    ],
  },
  pitfalls: [
    "**Alignment is your job.** The chart takes the position in the string as the column. A single insertion shifts everything after it and washes out the logo.",
    "**Gaps and other characters are ignored.** A dash or an N does not count as a letter in the column, which means a position full of gaps can look as conserved as a position full of data. Check the counts.",
    "**Few sequences, short logo.** The small-sample correction deliberately lowers the height when you have little data. Turn it off only if you know why.",
    "**Bits are against a uniform background.** There is no correction for an AT-rich or GC-rich genome, so a column of A in an AT-rich genome looks more informative than it is.",
    "**Units change the story.** With `Probability` the stack height is frequency, not information, and every column reaches the top.",
  ],
  related: [
    { slug: "heatmap", why: "When you want to show the counts or frequencies behind each position as a grid." },
    { slug: "histogram", why: "When you only need the distribution of one value, not one stack per position." },
    { slug: "circos", why: "When the sequences are whole genomes and the story is about links between positions." },
  ],
  media: [
    { kind: "youtube", id: "46vnUaMa8JQ", title: "Sequence LOGOs", channel: "Introduction to Bioinformatics by Arne Elofsson at Stockholm University", note: "University lecture segment explaining how sequence logos encode conservation in bits." },
    { kind: "youtube", id: "a9PhB_TeK6g", title: "How to Understand Sequence Logos", channel: "Nikolay's Genetics Lessons", note: "Short explainer on reading letter heights and positions in a sequence logo." },
  ],
}
