import { defineConfig } from "astro/config";

// Static site (DESIGN.md §6.2). SITE_URL is the public address (HUMAN: domain).
export default defineConfig({
  site: process.env.SITE_URL || "https://example.invalid",
  output: "static",
  trailingSlash: "always",
  build: { format: "directory" },
});
