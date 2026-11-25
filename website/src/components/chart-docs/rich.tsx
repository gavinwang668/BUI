import katex from "katex"
import { href } from "@/lib/href"

const token = /(\$[^$]+\$|`[^`]+`|\*\*[^*]+\*\*|\*[^*\s][^*]*\*|\[[^\]]+\]\([^)]+\))/g

const math = (source: string, display: boolean) => katex.renderToString(source, { throwOnError: false, displayMode: display })

export function Rich({ text }: { text: string }) {
  return (
    <>
      {text.split(token).map((part, index) => {
        if (!part) return null
        if (part.startsWith("$") && part.endsWith("$") && part.length > 2) {
          return <span key={index} dangerouslySetInnerHTML={{ __html: math(part.slice(1, -1), false) }} />
        }
        if (part.startsWith("`") && part.endsWith("`")) return <code key={index}>{part.slice(1, -1)}</code>
        if (part.startsWith("**") && part.endsWith("**")) {
          return (
            <strong key={index} className="font-semibold">
              <Rich text={part.slice(2, -2)} />
            </strong>
          )
        }
        if (part.startsWith("*") && part.endsWith("*") && part.length > 2) {
          return (
            <em key={index}>
              <Rich text={part.slice(1, -1)} />
            </em>
          )
        }
        const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/)
        if (link) {
          return link[2].startsWith("/") ? (
            <a key={index} href={href(link[2])} className="prose-link">
              <Rich text={link[1]} />
            </a>
          ) : (
            <a key={index} href={link[2]} target="_blank" rel="noopener noreferrer" className="prose-link">
              <Rich text={link[1]} />
            </a>
          )
        }
        return part
      })}
    </>
  )
}

export function Tex({ source }: { source: string }) {
  return <div className="my-4 overflow-x-auto" dangerouslySetInnerHTML={{ __html: math(source, true) }} />
}

export function Prose({ paragraphs, className = "" }: { paragraphs: string[]; className?: string }) {
  return (
    <div className={`flex flex-col gap-4 ${className}`}>
      {paragraphs.map((paragraph, index) => (
        <p key={index}>
          <Rich text={paragraph} />
        </p>
      ))}
    </div>
  )
}

export function Bullets({ items }: { items: string[] }) {
  return (
    <ul className="flex flex-col gap-3">
      {items.map((item, index) => (
        <li key={index} className="relative pl-5 before:absolute before:left-0 before:top-[0.72em] before:size-1.5 before:rotate-45 before:bg-primary before:content-['']">
          <Rich text={item} />
        </li>
      ))}
    </ul>
  )
}

export function Steps({ items }: { items: string[] }) {
  return (
    <ol className="flex flex-col gap-3">
      {items.map((item, index) => (
        <li key={index} className="grid grid-cols-[2rem_1fr] items-baseline gap-2">
          <span className="tabular text-sm font-semibold text-primary">{String(index + 1).padStart(2, "0")}</span>
          <span>
            <Rich text={item} />
          </span>
        </li>
      ))}
    </ol>
  )
}
