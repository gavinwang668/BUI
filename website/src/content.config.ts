import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";
import { languageIds, sectionLabels } from "./config/nav";

const docs = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/docs" }),
  schema: z.object({
    /** The page title: short, plain, no trailing period. */
    title: z.string(),
    /** One sentence for search results, link previews and the page lede. */
    description: z.string(),
    /** Which sidebar group the page belongs to (see src/config/nav.ts). */
    section: z.enum(sectionLabels),
    /** Position inside the section; lower comes first. Leave gaps (10, 20, 30). */
    order: z.number().default(100),
    /** stable: works today. preview: works, may change. coming-soon: not built yet. */
    status: z.enum(["stable", "preview", "coming-soon"]).default("stable"),
    /** Short tags shown next to the title, e.g. ["New"], ["Windows only"]. */
    badges: z.array(z.string()).default([]),
    /** Languages the page is about; shown as chips and used by search. */
    languages: z.array(z.enum(languageIds)).default([]),
    /** Optional shorter label for the sidebar. */
    sidebarLabel: z.string().optional(),
    /** Repository files this page is based on, shown as "Sources" at the bottom. */
    sources: z.array(z.string()).default([]),
    /** Work in progress: not built, not in the sidebar or search. */
    draft: z.boolean().default(false),
    /**
     * A page of its own: its URL ("benchmarks/"), drawn without the docs sidebar. Its /docs/ URL
     * redirects there, and every link to it goes there.
     */
    page: z.string().regex(/^[a-z0-9-]+\/$/).optional(),
    /** With `page`: a landing component shown above the page's text ("performance": the benchmark highlights and charts). */
    lead: z.enum(["performance"]).optional(),
  }),
});

/** The /panels pages (bui-panels), drawn in the addon shell as /charts is. */
const panels = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/panels" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    /** The sidebar group: the guide, or the reference pages. */
    group: z.enum(["Guide", "Reference"]),
    /** Position in the sidebar; lower comes first. */
    order: z.number(),
    sidebarLabel: z.string().optional(),
    /** Repository files this page is based on, shown as "Sources" at the bottom. */
    sources: z.array(z.string()).default([]),
  }),
});

export const collections = { docs, panels };
