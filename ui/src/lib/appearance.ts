export const DEFAULT_MONO = `"Cascadia Code", "SF Mono", ui-monospace, Menlo, Consolas, monospace`;

export const ACCENT_PRESETS = [
  { label: "Blue", value: "#2f6feb" },
  { label: "Purple", value: "#8250df" },
  { label: "Green", value: "#1a7f37" },
  { label: "Orange", value: "#bc4c00" },
  { label: "Pink", value: "#bf3989" },
  { label: "Gray", value: "#59636e" },
] as const;

const GENERIC_FAMILIES = new Set(["monospace", "ui-monospace", "serif", "sans-serif", "system-ui", "cursive", "fantasy"]);

export function normalizeHex(input: string | null | undefined): string | null {
  const raw = input?.trim().replace(/^#/, "").toLowerCase() ?? "";
  if (/^[0-9a-f]{3}$/.test(raw)) return `#${[...raw].map((c) => c + c).join("")}`;
  if (/^[0-9a-f]{6}$/.test(raw)) return `#${raw}`;
  return null;
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

const DARK_TEXT = "#1f2328";

export function readableOn(hex: string): "#ffffff" | typeof DARK_TEXT {
  const l = luminance(hex);
  const onDark = (l + 0.05) / (luminance(DARK_TEXT) + 0.05);
  const onWhite = 1.05 / (l + 0.05);
  return onDark > onWhite ? DARK_TEXT : "#ffffff";
}

export function accentTokens(color: string | null | undefined): Record<string, string> | null {
  const hex = normalizeHex(color);
  if (!hex) return null;
  const fg = readableOn(hex);
  return {
    "--accent": hex,
    "--accent-fg": fg,
    "--bg-selected-focus": hex,
    "--fg-selected-focus": fg,
    "--bg-selected": `color-mix(in srgb, ${hex} 22%, var(--bg))`,
  };
}

export function monoStack(font: string | null | undefined): string | null {
  const families = (font ?? "")
    .split(",")
    .map((f) => f.trim())
    .filter(Boolean)
    .map((f) => (GENERIC_FAMILIES.has(f) || /^["'].*["']$/.test(f) ? f : `"${f.replace(/"/g, "")}"`));
  return families.length ? `${families.join(", ")}, ${DEFAULT_MONO}` : null;
}

export function listRowHeight(density: "compact" | "comfortable" | undefined, largeTargets: boolean | undefined): number {
  if (largeTargets) return 36;
  return density === "compact" ? 24 : 30;
}
