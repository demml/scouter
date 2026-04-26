import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import svelte from "@astrojs/svelte";
import mermaid from "astro-mermaid";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";

export default defineConfig({
  site: "https://docs.demml.io",
  base: "/scouter",
  integrations: [
    mermaid({
      autoTheme: true,
      mermaidConfig: {
        theme: "base",
        themeVariables: {
          fontFamily: "Inter, ui-sans-serif, sans-serif",
          fontSize: "15px",
          primaryColor: "#f54d55",
          primaryTextColor: "#fff8f7",
          primaryBorderColor: "#f8b05d",
          lineColor: "#f8b05d",
          secondaryColor: "#272a34",
          secondaryTextColor: "#fff8f7",
          secondaryBorderColor: "#f8b05d",
          tertiaryColor: "#2e303e",
          tertiaryTextColor: "#fff8f7",
          tertiaryBorderColor: "#f8b05d",
          background: "transparent",
          mainBkg: "#f54d55",
          clusterBkg: "#272a34",
          clusterBorder: "#f8b05d",
          edgeLabelBackground: "#272a34",
          textColor: "#f1dfda",
        },
      },
    }),
    starlight({
      title: "Scouter",
      description:
        "Scouter monitors production ML and GenAI systems with drift detection, tracing, and agent evaluation.",
      logo: {
        src: "./src/assets/scouter-icon.svg",
        alt: "Scouter",
      },
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/demml/scouter",
        },
      ],
      customCss: [
        "@fontsource-variable/inter",
        "@fontsource/jetbrains-mono",
        "./src/styles/custom.css",
        "katex/dist/katex.min.css",
      ],
      expressiveCode: {
        themes: ["catppuccin-mocha", "catppuccin-mocha"],
      },
      sidebar: [
        { label: "Overview", link: "/" },
        { slug: "installation" },
        {
          label: "Server",
          items: ["server", "server/postgres"],
        },
        {
          label: "Monitoring",
          items: [
            "monitoring",
            {
              label: "PSI",
              items: [
                "monitoring/psi/quickstart",
                "monitoring/psi/drift-config",
                "monitoring/psi/drift-profile",
                "monitoring/psi/theory",
              ],
            },
            {
              label: "SPC",
              items: [
                "monitoring/spc/quickstart",
                "monitoring/spc/drift-config",
                "monitoring/spc/drift-profile",
                "monitoring/spc/theory",
              ],
            },
            {
              label: "Custom metrics",
              items: ["monitoring/custom/quickstart"],
            },
            "monitoring/inference",
          ],
        },
        {
          label: "Agents",
          items: [
            "agents/overview",
            "agents/offline-evaluation",
            "agents/online-evaluation",
            "agents/reading-results",
            "agents/eval-dataset",
            "agents/tasks",
            "agents/gates",
          ],
        },
        {
          label: "Bifrost",
          items: [
            "bifrost/overview",
            "bifrost/quickstart",
            "bifrost/writing-data",
            "bifrost/reading-data",
            "bifrost/schema",
          ],
        },
        {
          label: "Tracing",
          items: [
            "tracing/overview",
            "tracing/genai-semantics",
            "tracing/instrumentor",
            "tracing/storage-architecture",
          ],
        },
        {
          label: "Profiling",
          items: ["profiling/overview"],
        },
        {
          label: "Evaluation platform",
          items: [
            "evaluation-platform",
            "evaluation-platform/eval-profiles-and-tasks",
            "evaluation-platform/offline-evaluation",
            "evaluation-platform/online-evaluation",
            "evaluation-platform/observability-and-traces",
            "evaluation-platform/discussion-and-tradeoffs",
            "evaluation-platform/comparison",
          ],
        },
        {
          label: "Tech specs",
          collapsed: true,
          items: [
            "specs",
            "specs/data-archive",
            "specs/scouter-queue",
            "specs/scouter-grpc",
            "specs/genai-eval",
            "specs/eval-task-executor",
          ],
        },
        {
          label: "Architecture",
          collapsed: true,
          items: ["architecture/overview"],
        },
        {
          label: "API reference",
          collapsed: true,
          items: ["api"],
        },
      ],
    }),
    svelte(),
  ],
  markdown: {
    remarkPlugins: [remarkMath],
    rehypePlugins: [rehypeKatex],
  },
});
