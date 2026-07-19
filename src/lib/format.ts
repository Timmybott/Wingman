export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "–";
  if (bytes === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(Math.floor(Math.log2(bytes) / 10), units.length - 1);
  const value = bytes / 2 ** (10 * exponent);
  const rendered = value >= 100 || exponent === 0 ? Math.round(value).toString() : value.toFixed(1);
  return `${rendered} ${units[exponent]}`;
}

export function formatMib(mib: number): string {
  return formatBytes(mib * 1024 * 1024);
}

/** CPU usage relative to the server's limit; null when the limit is 0 (unlimited). */
export function cpuPercent(absolute: number, limit: number): number | null {
  if (limit <= 0) return null;
  return clampPercent((absolute / limit) * 100);
}

/** Memory usage relative to the limit (limit in MiB); null when unlimited. */
export function memoryPercent(bytes: number, limitMib: number): number | null {
  if (limitMib <= 0) return null;
  return clampPercent((bytes / (limitMib * 1024 * 1024)) * 100);
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, value));
}

// CSI sequences (colors, cursor movement) and OSC sequences (window titles),
// as emitted by game servers into their console output.
// eslint-disable-next-line no-control-regex
const ANSI_PATTERN = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g;

/** Strip ANSI escape sequences and trailing carriage returns from a console line. */
export function stripAnsi(line: string): string {
  return line.replace(ANSI_PATTERN, "").replace(/\r+$/, "");
}
