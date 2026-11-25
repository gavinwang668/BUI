import { readdirSync, writeFileSync } from "node:fs"
import { join, dirname } from "node:path"
import { fileURLToPath } from "node:url"

const folder = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "data", "charts")
const slugs = readdirSync(folder)
  .filter((file) => file.endsWith(".ts") && !["types.ts", "registry.ts", "domains.ts", "intents.ts", "loaders.ts", "docs.ts"].includes(file))
  .map((file) => file.slice(0, -3))
  .sort()

const lines = slugs.map((slug) => `  "${slug}": () => import("./${slug}"),`)
writeFileSync(
  join(folder, "loaders.ts"),
  `import type { ChartDoc } from "./types"\n\nexport const loaders: Record<string, () => Promise<{ doc: ChartDoc }>> = {\n${lines.join("\n")}\n}\n`,
)
console.log(`${slugs.length} content files`)
