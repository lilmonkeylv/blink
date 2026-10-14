const SHORTHAND = /^[\w.-]+\/[\w.-]+$/;

export function isShorthand(input: string): boolean {
  return SHORTHAND.test(input.trim());
}

export function cloneUrl(input: string): string {
  const trimmed = input.trim();
  return isShorthand(trimmed) ? `https://github.com/${trimmed.replace(/\.git$/, "")}.git` : trimmed;
}

export function repoNameFromUrl(url: string): string {
  const trimmed = url.trim().replace(/\/+$/, "").replace(/\.git$/, "");
  return trimmed.split(/[/:]/).pop() ?? "";
}
