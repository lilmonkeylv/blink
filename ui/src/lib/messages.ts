export interface SavedMessage {
  summary: string;
  description: string;
}

const MAX_RECENT = 20;
const key = (repo: string) => `blink.messages.${repo}`;

export function pushRecent(list: SavedMessage[], message: SavedMessage, max = MAX_RECENT): SavedMessage[] {
  const summary = message.summary.trim();
  if (!summary) return list;
  const entry = { summary, description: message.description.trim() };
  return [entry, ...list.filter((m) => m.summary !== summary)].slice(0, max);
}

export function recentMessages(repo: string): SavedMessage[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(key(repo)) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((m) => typeof m?.summary === "string") : [];
  } catch {
    return [];
  }
}

export function rememberMessage(repo: string, message: SavedMessage) {
  try {
    localStorage.setItem(key(repo), JSON.stringify(pushRecent(recentMessages(repo), message)));
  } catch {}
}
