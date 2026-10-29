import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "list",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:1420",
    channel: "msedge",
    viewport: { width: 1440, height: 900 },
    reducedMotion: "reduce",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm.cmd run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
  projects: [
    ...(process.env.CORTEXOS_BENCHMARK_ACCEPTANCE_REAL === "1"
      ? [{ name: "benchmark-acceptance-real", testMatch: "benchmark-acceptance-real.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_BENCHMARK_METRICS_REAL === "1"
      ? [{ name: "benchmark-metrics-real", testMatch: "benchmark-metrics-real.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_BENCHMARK_RUNNER === "1"
      ? [{ name: "benchmark-runner", testMatch: "benchmark-runner.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_BENCHMARK_PREFLIGHT === "1"
      ? [{ name: "benchmark-preflight", testMatch: "benchmark-preflight.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_DELIVERY_CHECK === "1"
      ? [{ name: "delivery", testMatch: "windows-delivery.spec.ts" }]
      : []),
    { name: "edge", testMatch: ["app.spec.ts", "graph-preview.spec.ts", "ui-usability.spec.ts", "cloud-connection.spec.ts", "personalized.spec.ts", "benchmark-result-drawer.spec.ts"] },
    {
      name: "desktop",
      testMatch: [
        "window-drag.spec.ts",
        "standard-benchmark.spec.ts",
        "benchmark-report-pages.spec.ts",
        "benchmark-report-usability.spec.ts",
        "desktop.spec.ts",
        "cloud-native.spec.ts",
        "prototype-ui.spec.ts",
        "ui-usability-native.spec.ts",
        "ui-micro-native.spec.ts",
        "chat.spec.ts",
        "markdown-camera.spec.ts",
        "chat-design.spec.ts",
        "context.spec.ts",
        "retrieval.spec.ts",
        "trace.spec.ts",
        "knowledge.spec.ts",
        "knowledge-layout.spec.ts",
        "sources.spec.ts",
        "citations.spec.ts",
        "graph.spec.ts",
        "graph-neighborhood.spec.ts",
        "graph-instances.spec.ts",
        "graph-query.spec.ts",
        "graph-exploration.spec.ts",
        "graph-reading.spec.ts",
      ],
    },
    ...(process.env.CORTEXOS_REAL_CONTEXT === "1"
      ? [{ name: "real-context", testMatch: "context-real.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_GRAPH_BASELINE === "1"
      ? [{ name: "graph-baseline", testMatch: "graph-baseline.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_REAL_OLLAMA === "1"
      ? [{ name: "ollama", testMatch: "ollama.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_REAL_GRAPH === "1"
      ? [{ name: "real-graph", testMatch: "graph-model.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_STREAM_PROFILE === "1"
      ? [{ name: "stream-profile", testMatch: "stream-profile.spec.ts" }]
      : []),
  ],
});
