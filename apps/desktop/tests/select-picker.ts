import { expect, type Locator } from "@playwright/test";

export async function expectPicker(select: Locator, open: boolean) {
  await expect.poll(() => select.evaluate(element => element.matches(":open"))).toBe(open);
}
