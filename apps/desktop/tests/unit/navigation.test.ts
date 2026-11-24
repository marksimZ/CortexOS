import assert from "node:assert/strict";
import test from "node:test";
import {
  navigationHash,
  navigationParams,
  parseNavigation,
} from "../../src/navigation.ts";

test("round trips a knowledge graph link back to the source chat message", () => {
  const back = navigationHash("chat", {
    workspace: "工作区 A",
    conversation: "conv-1",
    message: "msg-1",
  });
  const link = parseNavigation(
    navigationHash("graph", {
      workspace: "工作区 A",
      node: "node-1",
      back,
    }),
  );
  assert.equal(link.mode, "knowledge");
  assert.equal(link.node, "node-1");
  assert.equal(link.back, back);
  assert.equal(navigationParams(back).get("message"), "msg-1");
});

test("uses one query parser for embedded punctuation and rejects external return links", () => {
  const link = parseNavigation(
    "#/chat?conversation=question?2&citation=0&back=https%3A%2F%2Fexample.invalid",
  );
  assert.equal(link.conversation, "question?2");
  assert.equal(link.citation, 0);
  assert.equal(link.back, null);
  assert.equal(parseNavigation("#/benchmark?view=harness").page, "chat");
});
