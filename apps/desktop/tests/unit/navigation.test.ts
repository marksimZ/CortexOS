import assert from "node:assert/strict";
import test from "node:test";
import {
  navigationHash,
  navigationParams,
  parseNavigation,
} from "../../src/navigation.ts";

test("round trips an execution link back to the exact report and item", () => {
  const back = navigationHash("benchmark", {
    workspace: "工作区 A",
    view: "personalized",
    batch: "旧/测",
    item: 0,
    page0: 2,
  });
  const link = parseNavigation(
    navigationHash("graph", {
      workspace: "工作区 A",
      run: "run-1",
      mode: "execution",
      back,
    }),
  );
  assert.equal(link.mode, "execution");
  assert.equal(link.back, back);
  assert.equal(parseNavigation(back).benchmarkView, "personalized");
  assert.equal(navigationParams(back).get("item"), "0");
  assert.equal(navigationParams(back).get("page0"), "2");
});

test("uses one query parser for embedded punctuation and rejects external return links", () => {
  const link = parseNavigation(
    "#/chat?conversation=question?2&citation=0&back=https%3A%2F%2Fexample.invalid",
  );
  assert.equal(link.conversation, "question?2");
  assert.equal(link.citation, 0);
  assert.equal(link.back, null);
  assert.equal(
    parseNavigation("#/benchmark?view=harness").benchmarkView,
    "harness",
  );
  assert.equal(parseNavigation("#/benchmark?view=rag").benchmarkView, "rag");
});
