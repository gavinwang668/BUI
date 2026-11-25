export type Media =
  | { kind: "youtube"; id: string; title: string; channel: string; note: string }
  | { kind: "tweet"; url: string; author: string; text: string; note: string }

export type MathBlock = {
  heading: string
  body: string[]
  tex?: string[]
}

export type Story = {
  title: string
  paragraphs: string[]
}

export type Related = {
  slug: string
  why: string
}

export type ChartDoc = {
  tagline: string
  intro: string[]
  math: MathBlock[]
  use: string[]
  avoid: string[]
  read: string[]
  story: Story
  pitfalls: string[]
  related: Related[]
  media: Media[]
}
