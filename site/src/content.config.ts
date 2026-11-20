// Content collections (DESIGN.md §7.2): a typed schema catches a missing
// title or date at build time.
import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const docs = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.enum(["Start", "Everyday", "Tools", "Reference"]),
  }),
});

// The release notes: also the in-app "What's New" and the Debian changelog.
const changelog = defineCollection({
  loader: glob({ pattern: "*.md", base: "./src/content/changelog" }),
  schema: z.object({
    version: z.string(),
    date: z.coerce.date(),
    title: z.string(),
    summary: z.string(),
  }),
});

export const collections = { docs, changelog };
