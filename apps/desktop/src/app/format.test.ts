import { describe, it, expect } from "vitest";
import { percentage, duration, bytes } from "./format";
describe("progress reporting", () => {
  it("does not claim completion before validation", () => {
    expect(percentage(10, 10, "verifying")).toBe(99);
    expect(percentage(10, 10, "completed")).toBe(100);
    expect(percentage(0, 0, "downloading")).toBe(0);
  });
  it("formats long video durations and large files", () => {
    expect(duration(8700)).toBe("02:25:00");
    expect(bytes(9173376289)).toBe("8.5 GB");
  });
});
