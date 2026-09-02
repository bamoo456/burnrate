import { defineConfig } from "vitepress";

const SITE_URL = "https://bamoo456.github.io/burnrate/";

export default defineConfig({
  title: "Burnrate",
  description:
    "Menu-bar usage monitor for Claude Code, Codex, GitHub Copilot, Antigravity, OpenCode Go, OpenRouter, Runpod, and AWS — quotas, credits, spend, and subscription limits at a glance.",
  base: "/burnrate/",
  lang: "en-US",
  cleanUrls: true,
  lastUpdated: true,
  appearance: "force-dark",
  sitemap: { hostname: SITE_URL },

  head: [
    ["link", { rel: "icon", type: "image/png", href: "/burnrate/favicon.png" }],
    ["meta", { name: "theme-color", content: "#f36c3d" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:title", content: "Burnrate" }],
    [
      "meta",
      {
        property: "og:description",
        content:
          "Know your burn. A menu-bar monitor for AI provider quotas, credits, and spend.",
      },
    ],
    ["meta", { property: "og:url", content: SITE_URL }],
    [
      "meta",
      {
        property: "og:image",
        content: `${SITE_URL}screenshots/preferences.png`,
      },
    ],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
  ],

  themeConfig: {
    logo: "/logo.svg",

    nav: [
      {
        text: "Guide",
        link: "/guide/what-is-burnrate",
        activeMatch: "/guide/",
      },
      {
        text: "Providers",
        link: "/providers/claude-code",
        activeMatch: "/providers/",
      },
      {
        text: "Download",
        link: "https://github.com/bamoo456/burnrate/releases/latest",
      },
    ],

    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "What is Burnrate?", link: "/guide/what-is-burnrate" },
          { text: "Installation", link: "/guide/installation" },
          { text: "Getting Started", link: "/guide/getting-started" },
          { text: "Configuration", link: "/guide/configuration" },
          { text: "Troubleshooting", link: "/guide/troubleshooting" },
        ],
      },
      {
        text: "Providers",
        items: [
          { text: "Claude Code", link: "/providers/claude-code" },
          { text: "Codex", link: "/providers/codex" },
          { text: "GitHub Copilot", link: "/providers/github-copilot" },
          { text: "Antigravity", link: "/providers/antigravity" },
          { text: "OpenCode Go", link: "/providers/opencode-go" },
          { text: "OpenRouter", link: "/providers/openrouter" },
          { text: "Runpod", link: "/providers/runpod" },
          { text: "AWS", link: "/providers/aws" },
        ],
      },
    ],

    socialLinks: [
      { icon: "github", link: "https://github.com/bamoo456/burnrate" },
    ],

    search: { provider: "local" },

    editLink: {
      pattern: "https://github.com/bamoo456/burnrate/edit/main/website/:path",
      text: "Edit this page on GitHub",
    },

    footer: {
      message: "Released under the MIT License.",
      copyright:
        "Copyright © 2025-present James Brink · fork maintained by bamoo456",
    },
  },
});
