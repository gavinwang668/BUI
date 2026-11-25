import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "The two backbone angles of every amino-acid residue, on one square that shows which folds are possible.",
  intro: [
    "A protein is a chain, and a chain can only bend where it has joints. Along the backbone each residue has two joints that matter, rotations called phi and psi. Spin them freely and you would expect any pair of angles to be possible. They are not. Atoms get in each other's way, so most of the square from minus 180 to plus 180 degrees is off limits, and the allowed patches are the shapes that proteins are actually built from.",
    "[G. N. Ramachandran](https://en.wikipedia.org/wiki/Ramachandran_plot) drew this in 1963 with his colleagues C. Ramakrishnan and V. Sasisekharan at the University of Madras, working out by hand and by hard-sphere geometry which angle pairs avoid steric clashes. The idea was to predict from geometry which regions should be full and which empty, and as measured structures piled up the map held.",
    "That is why the plot is still the first sanity check on a structure. If a refined model has residues in the forbidden zones, either the protein is doing something interesting or someone has made a mistake, and statistically the second is much more likely.",
  ],
  math: [
    {
      heading: "What phi and psi are",
      body: [
        "Both are [dihedral](https://en.wikipedia.org/wiki/Dihedral_angle) (torsion) angles. For four consecutive atoms with bond vectors $b_1, b_2, b_3$, the angle between the plane through the first three and the plane through the last three is the quantity below. For residue $i$, $\\varphi$ is measured about the N to C$\\alpha$ bond using atoms C$_{i-1}$, N$_i$, C$\\alpha_i$, C$_i$, and $\\psi$ about C$\\alpha$ to C using N$_i$, C$\\alpha_i$, C$_i$, N$_{i+1}$. This chart does not compute these; you supply the angles, in degrees, from your structure software.",
      ],
      tex: [
        "n_1 = b_1\\times b_2,\\quad n_2 = b_2\\times b_3,\\qquad \\theta = \\operatorname{atan2}\\!\\left(\\frac{(n_1\\times n_2)\\cdot b_2}{\\lVert b_2\\rVert},\\; n_1\\cdot n_2\\right)",
      ],
    },
    {
      heading: "What the chart does with them",
      body: [
        "Each residue becomes one point at $(\\varphi, \\psi)$. Both axes are fixed to $[-180^{\\circ}, 180^{\\circ}]$ with tick labels in degrees, whatever your data contain, so the square is always the whole square and a point near one edge is really near the opposite one too: the plane is a torus, and an angle of $-179^{\\circ}$ is two degrees from $+179^{\\circ}$, not 358.",
        "Nothing is binned, smoothed or classified. The chart does not draw the allowed regions for you. The density of your own points is the picture. That is deliberate, because the allowed regions depend on the residue type and the reference set you trust.",
      ],
    },
    {
      heading: "Landmarks to expect",
      body: [
        "In a typical protein, residues gather in three places: the right-handed [alpha helix](https://en.wikipedia.org/wiki/Alpha_helix) near $(-60^{\\circ}, -45^{\\circ})$, the beta-sheet region in the upper left around $(-120^{\\circ}, +130^{\\circ})$, and a smaller patch for the left-handed helix near $(+60^{\\circ}, +45^{\\circ})$. Glycine has no side chain and wanders almost anywhere; proline's ring locks $\\varphi$ near $-63^{\\circ}$.",
      ],
    },
  ],
  use: [
    "You want a first quality check on a protein model, a crystal structure or a simulation snapshot: are the angles in plausible places?",
    "You want to show the secondary-structure content of a protein at a glance. A helical protein piles up in one patch, a beta-barrel in another.",
    "You are following a simulation and want to see which residues moved from one region to another.",
    "You teach structural biology and want students to see why helices and sheets exist.",
  ],
  avoid: [
    "You need a verdict on every residue. The chart shows a cloud, not a list of violations. Use a validation tool for per-residue outliers.",
    "You are comparing proteins of very different sizes by eye. A small protein makes a sparse plot, and the shape means less.",
    "You want to separate glycine, proline and general residues. Draw one chart per class, since their allowed regions differ.",
    "Your angles are in radians. The axes are labelled in degrees and will show a tiny dot in the middle.",
  ],
  read: [
    "Look for the three main clusters and name them: alpha helix on the left towards the bottom, beta sheet in the top left, left-handed region on the right.",
    "Notice the empty areas. They are not missing data but forbidden territory. A lone point in one is worth looking up.",
    "Check the edges. A cluster that appears to be cut at $\\pm 180^{\\circ}$ continues on the other side.",
    "Hover a point to read its exact angles, then trace it back to the residue in your own data.",
  ],
  story: {
    title: "A structure that was too good to be true",
    paragraphs: [
      "Lucía was a crystallographer finishing a model of an enzyme that had resisted three previous attempts by her lab, a record that the lab kept on a whiteboard under the heading Previous Attempts. The data were at 2.1 angstroms, the fit statistics were improving, and she was one deposition form away from not thinking about it ever again.",
      "Her refinement software said everything was converging nicely. That is exactly the kind of sentence that precedes a rejection. A refinement that fits the electron density can still be geometrically strange, because density maps are generous with the shape of a side chain and silent about the polite angle of a backbone.",
      "She needed an independent, picture-sized opinion on whether the backbone was plausible, one that did not depend on how well the structure fitted the data. She had the angles for all 412 residues from the model file, and a plot that has been the standard for that question for sixty years.",
      "She loaded them into `<Ramachandran>`. The picture was reassuring in the first glance: a tight, dense blob in the alpha-helix region, a long smear in the beta region, a few scattered points on the right. Then she looked at the empty square on the lower right, the one that should be all but deserted, and there were nine points in it.",
      "Nine of 412 is two percent, more than a clean structure would tolerate. She hovered over each one and noted the angles. Seven of them belonged to one loop between two helices, and the loop sat in the weakest part of the density. The software had stretched the loop to fit the map and the backbone had paid for it.",
      "She rebuilt the loop by hand, rotating three residues to sit in allowed regions, and refined again. The fit statistics barely moved, but the number of outliers fell from nine to one, a glycine that was entitled to be there. The structure went to the database and nobody reported a problem with it, which is the highest praise the process allows.",
      "She took the old whiteboard heading down and wrote Previous Attempts: 3, Current Attempt: Fine.",
    ],
  },
  pitfalls: [
    "**Radians versus degrees.** The axes are fixed from minus 180 to plus 180 degrees. Convert before plotting.",
    "**Forgetting the wrap.** The left and right edges are the same angle, as are the top and bottom. A cluster that is sliced by the border is one cluster.",
    "**Mixing residue types.** Glycine and proline have different allowed regions from everything else. A single combined plot hides outliers of one type inside the permitted area of another.",
    "**Treating density as quality.** A beautiful Ramachandran plot is necessary, not sufficient. Models can be refined into the allowed regions while the structure is still wrong.",
  ],
  related: [
    { slug: "xy-chart", why: "When you want a general scatter without the fixed angular axes." },
    { slug: "sequence-logo", why: "When you want to see which amino acids occupy each position in an alignment." },
    { slug: "hodograph", why: "When the angle is a direction in a flow and you want the path through successive values, not a cloud." },
  ],
  media: [
    {kind:"youtube",id:"aO0l1PReGo0",title:"How to Interpret Ramachandran Plots",channel:"Catalyst University",note:"Walks through the allowed and forbidden regions for phi and psi angles."},
    {kind:"youtube",id:"JyUMLSsbecI",title:"Understanding PHI and PSI Angles, Ramachandran Plots, & Newman Projections",channel:"Molecular Memory",note:"Builds intuition for the backbone dihedral angles the plot is drawn from."},
  ],
}
