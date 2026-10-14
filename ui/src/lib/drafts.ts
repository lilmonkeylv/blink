export interface CommitDraft {
  summary: string;
  description: string;
  coAuthors: string;
  showCoAuthors: boolean;
  amend: boolean;
}

export const EMPTY_DRAFT: CommitDraft = { summary: "", description: "", coAuthors: "", showCoAuthors: false, amend: false };

const KEY = "blink.drafts";

export function isBlankDraft(d: CommitDraft): boolean {
  return !d.summary && !d.description && !d.coAuthors && !d.showCoAuthors && !d.amend;
}

function asDraft(value: unknown): CommitDraft | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  const text = (k: string) => (typeof v[k] === "string" ? (v[k] as string) : "");
  return {
    summary: text("summary"),
    description: text("description"),
    coAuthors: text("coAuthors"),
    showCoAuthors: v.showCoAuthors === true,
    amend: v.amend === true,
  };
}

export function parseDrafts(raw: string | null): Record<string, CommitDraft> {
  try {
    const parsed = JSON.parse(raw ?? "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const out: Record<string, CommitDraft> = {};
    for (const [repo, value] of Object.entries(parsed)) {
      const draft = asDraft(value);
      if (draft && !isBlankDraft(draft)) out[repo] = draft;
    }
    return out;
  } catch {
    return {};
  }
}

export function withDraft(all: Record<string, CommitDraft>, repo: string, draft: CommitDraft): Record<string, CommitDraft> {
  const rest = { ...all };
  delete rest[repo];
  return isBlankDraft(draft) ? rest : { ...rest, [repo]: draft };
}

export function savedDrafts(): Record<string, CommitDraft> {
  try {
    return parseDrafts(localStorage.getItem(KEY));
  } catch {
    return {};
  }
}

export function persistDraft(repo: string, draft: CommitDraft) {
  try {
    localStorage.setItem(KEY, JSON.stringify(withDraft(savedDrafts(), repo, draft)));
  } catch {}
}
