import { benchmark } from "../../src/api/benchmark";
import { chat } from "../../src/api/chat";
import { knowledge } from "../../src/api/knowledge";
import type { Message } from "../../src/studio";

// Compiled by tsc. These calls document mismatches the frontend must reject.
if (false) {
  const messages: Promise<Message[]> = chat("load_messages", {
    workspaceId: "w",
    conversationId: "c",
  });
  void messages;
  // @ts-expect-error The workspace is part of the ownership check.
  void chat("load_messages", { conversationId: "c" });
  // @ts-expect-error Native commands use the registered name.
  void knowledge("load_graph_edges", { workspaceId: "w", nodeIds: ["n"] });
  const invalidReport = {
    workspaceId: "w",
    batchId: "b",
    scope: "chat" as const,
  };
  // @ts-expect-error Report scope must match a backend purpose.
  void benchmark("get_benchmark_overview", invalidReport);
}
