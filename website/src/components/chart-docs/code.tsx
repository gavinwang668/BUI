// the code arrives highlighted (scripts/gen-charts.mjs runs shiki), so this renders without a highlighter
export function Code({ html: markup, label }: { html: string; label?: string }) {
  return (
    <figure className="overflow-hidden border border-border">
      {label && <figcaption className="border-b border-border bg-(--surface-2) px-4 py-2 font-mono text-xs text-(--muted)">{label}</figcaption>}
      <div className="code-block bg-(--code-bg)" dangerouslySetInnerHTML={{ __html: markup }} />
    </figure>
  )
}
