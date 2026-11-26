// The /panels pages (bui-panels): their URLs and reading order.
import type { CollectionEntry } from "astro:content";
import { href } from "./href";

export type PanelPage = CollectionEntry<"panels">;

/** The guide (index.mdx) is /panels/; the others are /panels/<file>/. */
export const panelHref = (page: PanelPage) => href(page.id === "index" ? "panels/" : `panels/${page.id}/`);

/** Reading order: the guide first, then the reference pages, each group by `order`. */
export const panelsInOrder = (pages: PanelPage[]) =>
  [...pages].sort((a, b) => Number(a.data.group === "Reference") - Number(b.data.group === "Reference") || a.data.order - b.data.order);
