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
  webServer: process.env.CORTEXOS_NATIVE_ONLY === "1" ? undefined : {
    command: "npm.cmd run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
  projects: [
    ...(process.env.CORTEXOS_H1_BASELINE === "1" ? [{ name: "h1-baseline", testMatch: "h1-baseline.spec.ts" }] : []),
    ...(process.env.CORTEXOS_H3_BASELINE === "1" ? [{ name: "h3-baseline", testMatch: "h3-baseline.spec.ts" }] : []),
    ...(process.env.CORTEXOS_HARNESS_EXPERIMENT === "1" ? [{name:"harness-experiment",testMatch:"harness-experiment.spec.ts"}] : []),
    ...(process.env.CORTEXOS_HARNESS_AIDER_SESSION === "1" ? [{name:"harness-aider-session",testMatch:"harness-aider-session.spec.ts"}] : []),
    ...(process.env.CORTEXOS_HARNESS_AIDER_RUNTIME === "1" ? [{name:"harness-aider-runtime",testMatch:"harness-aider-runtime.spec.ts"}] : []),
    ...(process.env.CORTEXOS_HARNESS_CLAUDE_SESSION === "1" ? [{name:"harness-claude-session",testMatch:"harness-claude-session.spec.ts"}] : []),
    ...(process.env.CORTEXOS_HARNESS_CLAUDE_RUNTIME === "1"
      ? [{ name: "harness-claude-runtime", testMatch: "harness-claude-runtime.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_HARNESS_SESSION === "1"
      ? [{ name: "harness-codex-session", testMatch: "harness-codex-session.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_HARNESS_CODEX_RUNTIME_REAL === "1"
      ? [{ name: "harness-codex-runtime", testMatch: "harness-codex-runtime.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_HARNESS_WORKSPACE_REAL === "1"
      ? [{ name: "harness-workspace-real", testMatch: "harness-workspace.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_HARNESS_PREFLIGHT === "1"
      ? [{ name: "harness-preflight", testMatch: "harness-preflight.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_HARNESS_DOCKER_REAL === "1"
      ? [{ name: "harness-docker-real", testMatch: "harness-programming-validation.spec.ts" }]
      : []),
    ...(process.env.CORTEXOS_BENCHMARK_PYTHON_REAL === "1"
      ? [{ name: "benchmark-python-real", testMatch: "benchmark-python-real.spec.ts" }]
      : []),
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
    { name: "edge", testMatch: ["h1-correctness.spec.ts", "app.spec.ts", "ui-usability.spec.ts", "cloud-connection.spec.ts", "personalized.spec.ts", "benchmark-result-drawer.spec.ts", "knowledge-scope.spec.ts", "rag-evaluation.spec.ts", "local-runtime.spec.ts", "runtime-cloud.spec.ts", "runtime-profiles.spec.ts"] },
    {
      name: "desktop",
      testMatch: [
        "h1-correctness.spec.ts",
        "harness-fit.spec.ts",
        "harness-programming.spec.ts",
        "harness-tasks.spec.ts",
        "window-drag.spec.ts",
        "standard-benchmark.spec.ts",
        "benchmark-report-pages.spec.ts",
        "benchmark-report-usability.spec.ts",
        "desktop.spec.ts",
        "cloud-native.spec.ts",
        "prototype-ui.spec.ts",
        "ui-usability-native.spec.ts",
        "ui-micro-native.spec.ts",
        "storage-native.spec.ts",
        "storage-recovery-native.spec.ts",
        "diagnostics-native.spec.ts",
        "updates-native.spec.ts",
        "data-portability-native.spec.ts",
        "h5-product-native.spec.ts",
        "reminders-native.spec.ts",
        "personal-assistant-native.spec.ts",
        "generation-style-native.spec.ts",
        "thinking-stream-native.spec.ts",
        "web-url-import-native.spec.ts",
        "chat-search-native.spec.ts",
        "page-citation-native.spec.ts",
        "web-search-native.spec.ts",
        "chat.spec.ts",
        "markdown-camera.spec.ts",
        "chat-design.spec.ts",
        "context.spec.ts",
        "retrieval.spec.ts",
        "rag-native.spec.ts",
        "trace.spec.ts",
        "knowledge.spec.ts",
        "knowledge-layout.spec.ts",
        "sources.spec.ts",
        "citations.spec.ts",
        "graph.spec.ts",
        "graph-neighborhood.spec.ts",
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
