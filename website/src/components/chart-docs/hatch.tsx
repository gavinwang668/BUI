export function Hatch({ className = "h-10" }: { className?: string }) {
  return <div aria-hidden="true" className={`hatch border-b border-border ${className}`} />
}

export function Section({ id, title, children, kicker }: { id: string; title: string; kicker?: string; children: React.ReactNode }) {
  return (
    <section id={id} className="scroll-mt-28 border-b border-border">
      <div className="px-4 py-8 sm:px-8 md:py-12">
        {kicker && <p className="mb-2 text-xs font-semibold uppercase tracking-[0.16em] text-primary">{kicker}</p>}
        <h2 className="mb-6 text-2xl font-medium uppercase tracking-tight text-balance md:text-3xl">{title}</h2>
        {children}
      </div>
    </section>
  )
}
