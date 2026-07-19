import { describe, expect, it } from "vitest";
import { cpuPercent, formatBytes, formatMib, memoryPercent, stripAnsi } from "./format";

describe("formatBytes", () => {
  it("formats zero and small values without decimals", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("scales to binary units with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KiB");
    expect(formatBytes(1.5 * 1024 * 1024)).toBe("1.5 MiB");
    expect(formatBytes(1_610_612_736)).toBe("1.5 GiB");
  });

  it("drops decimals for three-digit values", () => {
    expect(formatBytes(512 * 1024 * 1024)).toBe("512 MiB");
  });

  it("returns a dash for invalid input", () => {
    expect(formatBytes(-1)).toBe("–");
    expect(formatBytes(Number.NaN)).toBe("–");
  });
});

describe("formatMib", () => {
  it("treats input as MiB", () => {
    expect(formatMib(4096)).toBe("4.0 GiB");
  });
});

describe("cpuPercent", () => {
  it("is relative to the limit", () => {
    expect(cpuPercent(100, 200)).toBe(50);
  });

  it("returns null for unlimited (0) limits", () => {
    expect(cpuPercent(87.5, 0)).toBeNull();
  });

  it("clamps to 100 when usage exceeds the limit", () => {
    expect(cpuPercent(250, 200)).toBe(100);
  });
});

describe("memoryPercent", () => {
  it("compares bytes against a MiB limit", () => {
    expect(memoryPercent(2048 * 1024 * 1024, 4096)).toBe(50);
  });

  it("returns null for unlimited limits", () => {
    expect(memoryPercent(123, 0)).toBeNull();
  });
});

describe("stripAnsi", () => {
  it("removes color codes", () => {
    expect(stripAnsi("\x1b[33m[mock]\x1b[0m Container booted")).toBe(
      "[mock] Container booted",
    );
  });

  it("removes cursor and OSC title sequences", () => {
    expect(stripAnsi("\x1b[2J\x1b[H\x1b]0;My Server\x07Done (2.1s)!")).toBe("Done (2.1s)!");
  });

  it("removes trailing carriage returns", () => {
    expect(stripAnsi("Loading libraries, please wait...\r")).toBe(
      "Loading libraries, please wait...",
    );
  });

  it("leaves plain lines untouched", () => {
    expect(stripAnsi("[12:00:01] [Server thread/INFO]: Done")).toBe(
      "[12:00:01] [Server thread/INFO]: Done",
    );
  });
});
