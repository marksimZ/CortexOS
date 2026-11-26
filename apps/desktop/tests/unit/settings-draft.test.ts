import assert from "node:assert/strict";
import test from "node:test";
import { rebaseSettingsDraft } from "../../src/settings-draft.ts";

const original = {
  model: "original",
  numCtx: 4096,
  temperature: 0.3,
  topP: 0.9,
  numPredict: 512,
  timeoutSeconds: 180,
  deepThinking: false,
};

test("reload updates untouched fields and keeps edits including an empty numeric field", () => {
  const draft = { ...original, model: "draft", numPredict: Number.NaN };
  const saved = { ...original, model: "saved", numCtx: 8192, numPredict: 1024 };
  assert.deepEqual(rebaseSettingsDraft(draft, original, saved), {
    ...saved,
    model: "draft",
    numPredict: Number.NaN,
  });
});

test("save commits the submitted values and retains edits made while saving", () => {
  const submitted = { ...original, model: "submitted" };
  const draft = { ...submitted, timeoutSeconds: 240 };
  const saved = { ...submitted, model: "normalized" };
  assert.deepEqual(rebaseSettingsDraft(draft, submitted, saved), {
    ...saved,
    timeoutSeconds: 240,
  });
});
