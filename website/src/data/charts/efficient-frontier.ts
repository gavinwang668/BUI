import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Every portfolio you could build from your assets, with the best ones traced as a curve.",
  intro: [
    "Ask three investors how to split money between stocks, bonds and gold and you will get four answers and one of them will mention a cousin. [Harry Markowitz](https://en.wikipedia.org/wiki/Harry_Markowitz) asked a sharper question in his 1952 paper on portfolio selection: for a given amount of risk you are willing to carry, what is the most return you can get? The answer is not a single portfolio. It is a curve, and everything below the curve is a portfolio that someone could have beaten without taking on any extra risk.",
    "That curve is the efficient frontier. It works because assets do not move in lockstep: two risky things that zig and zag at different times make a combination steadier than either. The chart's job is to put that on one page. Each asset appears as a square, a cloud of random portfolios fills the space between and around them, and the frontier runs along the top-left edge of the cloud like a roof.",
    "Markowitz shared the 1990 Nobel Prize in economics, partly for this picture. It is one of the rare charts where the single most valuable thing is an empty region: the area above and to the left of the curve, where nothing is available at any price.",
  ],
  math: [
    {
      heading: "Risk and return of a mix",
      body: [
        "Let $\\mu$ be the vector of expected returns you pass in (`returns`) and $\\Sigma$ the covariance matrix (`covariance`). A portfolio is a weight vector $w$ that sums to one. Its expected return is linear in the weights; its risk is the square root of a quadratic form, and the off-diagonal entries of $\\Sigma$ are where diversification lives.",
      ],
      tex: [
        "\\mu_p = w^{\\top}\\mu, \\qquad \\sigma_p = \\sqrt{w^{\\top}\\Sigma\\,w}, \\qquad \\sum_i w_i = 1",
        "S = \\frac{\\mu_p - r_f}{\\sigma_p}",
      ],
    },
    {
      heading: "The cloud of portfolios",
      body: [
        "The scatter is long-only. Each of the `portfolios` points (2000 by default) draws weights $w_i \\propto -\\ln u_i$ from a deterministic pseudo-random sequence, then divides by their sum. That is a uniform draw from the set of non-negative weights summing to one, so the cloud is not biased towards equal splits the way averaging uniform numbers would be. The points are coloured by the [Sharpe ratio](https://en.wikipedia.org/wiki/Sharpe_ratio) $S$ above, with $r_f$ taken from `risk_free` (zero if absent). The sequence is fixed, so the same inputs always give the same cloud.",
      ],
    },
    {
      heading: "The frontier and the tangent portfolio",
      body: [
        "The curve is not read off the cloud. It is solved in closed form. With $\\mathbf 1$ a vector of ones, define three scalars, and the minimum variance for a target return $m$ follows. The chart evaluates it at 101 evenly spaced targets, from the return of the global minimum-variance portfolio, $A/C$, up to 1.2 times the best single-asset return.",
      ],
      tex: [
        "A = \\mathbf 1^{\\top}\\Sigma^{-1}\\mu,\\quad B = \\mu^{\\top}\\Sigma^{-1}\\mu,\\quad C = \\mathbf 1^{\\top}\\Sigma^{-1}\\mathbf 1,\\quad D = BC - A^{2}",
        "\\sigma^{2}(m) = \\frac{C\\,m^{2} - 2A\\,m + B}{D}",
        "w_{\\text{tan}} = \\frac{\\Sigma^{-1}(\\mu - r_f\\mathbf 1)}{\\mathbf 1^{\\top}\\Sigma^{-1}(\\mu - r_f\\mathbf 1)}",
      ],
    },
    {
      heading: "What the closed form assumes",
      body: [
        "The frontier allows negative weights (short positions) because that is where the algebra is clean, while the random cloud does not. So the cloud sits inside the curve and touches it only when the best long-only portfolio happens to be the unconstrained one. The red dot is $w_{\\text{tan}}$, the portfolio with the highest Sharpe ratio, and the dashed [capital market line](https://en.wikipedia.org/wiki/Capital_market_line) runs from $(0, r_f)$ through it, extended to 1.4 times its risk. If $\\Sigma$ is singular, or $D \\le 0$, the frontier is skipped and only the cloud and the assets are drawn. Risk is shown on the x axis, return on y, both as percentages.",
      ],
    },
  ],
  use: [
    "You are choosing weights for three to a dozen assets and want to see how much diversification actually buys.",
    "You want to show a client or a committee that their current mix sits well inside the curve: the same return was available for less risk.",
    "You are comparing the effect of a changed assumption, such as a higher expected return for one asset, by watching the frontier move.",
    "You want a feel for the shape of the opportunity, not a single recommended answer.",
  ],
  avoid: [
    "Your expected returns are guesses with false confidence. The frontier is extremely sensitive to $\\mu$, and the optimiser will happily load up on whichever asset you were optimistic about.",
    "You need a long-only frontier. The curve here allows shorting, for constrained portfolios use the cloud's upper edge as a rough guide and solve the constraints elsewhere.",
    "You have hundreds of assets. The closed form needs a clean inverse of $\\Sigma$, and the picture stops being legible long before the algebra fails.",
    "Risk to you is not variance. Downside, drawdown and tail risk are different questions with different charts.",
  ],
  read: [
    "Find the leftmost point of the frontier: the global minimum-variance portfolio. Nothing has less risk than this, whatever the return.",
    "Look at the squares. A single asset far to the right of the frontier is paying for risk it does not need to carry.",
    "Follow the dashed capital market line to where it touches the curve: the red dot is the best return per unit of risk, given your risk-free rate.",
    "Hover a cloud point for its risk, return and Sharpe ratio. The colour gradient shows how the ratio climbs towards the upper left.",
  ],
  story: {
    title: "The pension that was already on the roof",
    paragraphs: [
      "Ines chaired the investment committee of a small teachers' pension fund, which is a job with all of the responsibility of a bank and none of the staff. The fund held three things: a domestic equity index, a global bond fund and a slice of infrastructure. It had held them in the same proportions since a trustee named Gerald had written them on a napkin in 2011. The napkin was treated with a respect usually reserved for tablets.",
      "A new actuary asked a quiet question at the March meeting: was the napkin portfolio any good? Nobody knew how to answer. The fund's consultant said the mix was reasonable, which is what consultants say about mixes, and Gerald said it had always been fine, which is what everyone says about a napkin.",
      "What Ines needed was a way to see the napkin portfolio against everything else that three assets can produce, using the fund's own estimates of return and its own covariance matrix, and a picture the trustees could read without a course.",
      "She gave `<EfficientFrontier>` the three expected returns, the covariance matrix from ten years of monthly data and a risk-free rate of 2.5 percent. Two thousand grey-to-yellow portfolios filled the screen, and the frontier arched above them. She added the napkin mix as a point and it sat in the middle of the cloud, a good way inside the curve.",
      "The same 6.1 percent expected return was available at 8.9 percent risk, and the napkin was carrying 11.4. Alternatively, holding the napkin's risk could have earned almost a point more. Put that way, the trustees stopped defending it. Gerald asked, with real dignity, whether the napkin could keep a smaller share. It could not.",
      "The fund moved towards the tangent portfolio over three quarters, trimming domestic equity and raising infrastructure. In the next year it delivered the same return with measured volatility about a quarter lower, and the pension committee's longest meeting of the year was cut to one hour because there was nothing left to argue about.",
      "The napkin now hangs framed in the boardroom, not as a guide but as a before picture.",
    ],
  },
  pitfalls: [
    "**Garbage in, optimiser out.** Small changes in expected returns move the frontier a lot. Treat the tangent portfolio as a thought experiment, not a trade list.",
    "**Units must match.** Returns, covariance and the risk-free rate must all be per the same period, for example annual. Mixing monthly covariances with annual returns shrinks the risk axis by $\\sqrt{12}$ and flatters everything.",
    "**Shorting is allowed on the curve.** The cloud is long-only. If the curve rises far above the cloud's edge, the extra return comes from negative weights you may not be allowed to hold.",
    "**A bad covariance matrix.** It must be symmetric and positive definite. A matrix assembled by hand from rounded correlations often is not, and the frontier will quietly vanish.",
  ],
  related: [
    { slug: "xy-chart", why: "When you only want a scatter of risk against return without the frontier and tangent portfolio." },
    { slug: "control-chart", why: "When risk is about a process drifting out of limits rather than a mix of assets." },
    { slug: "waterfall", why: "When you want to explain how a portfolio's return was built up, contribution by contribution." },
  ],
  media: [
    { kind: "youtube", id: "pwyR9uAM0iU", title: "Efficient Frontier and Portfolio Optimization Explained | The Ultimate Guide", channel: "Ryan O'Connell, CFA, FRM", note: "In-depth explanation of the efficient frontier and how it is computed." },
    { kind: "youtube", id: "PiXrLGMZr1g", title: "Investopedia Video: Efficient Froniter", channel: "Investopedia", note: "Short, plain-language overview of the concept." },
  ],
}
