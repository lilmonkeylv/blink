import { createResource, createSignal, For, Match, Show, Switch } from "solid-js";
import { api, type DiffLine, type DraftComment, type ReviewEvent, type ReviewFile } from "../../api";
import { relativeTime } from "../../lib/time";
import { notify, reportError, store } from "../../store";
import { Dialog } from "../Dialog";
import { Loading } from "../Loading";
import { DiffView } from "../DiffView";
import { Markdown } from "../Markdown";
import { closeDialog } from "./common";

const EVENT_LABELS: Record<ReviewEvent, string> = {
  COMMENT: "Commented",
  APPROVE: "Approved",
  REQUEST_CHANGES: "Changes requested",
};

const REVIEW_STATES: Record<string, string> = {
  APPROVED: "approved",
  CHANGES_REQUESTED: "requested changes",
  COMMENTED: "reviewed",
  DISMISSED: "had a review dismissed",
};

function lineTarget(path: string, line: DiffLine): Omit<DraftComment, "body"> | null {
  if (line.kind === "removed") return line.oldNo ? { path, line: line.oldNo, side: "LEFT" } : null;
  return line.newNo ? { path, line: line.newNo, side: "RIGHT" } : null;
}

const sameSpot = (a: Omit<DraftComment, "body">, b: Omit<DraftComment, "body">) =>
  a.path === b.path && a.line === b.line && a.side === b.side;

export function ReviewDialog(p: { number: number; title: string }) {
  const [data, { refetch }] = createResource(
    () => store.repo?.id ?? null,
    (id) => api.pullRequestReview(id, p.number),
  );
  const [pane, setPane] = createSignal<string | null>(null);
  const [drafts, setDrafts] = createSignal<DraftComment[]>([]);
  const [target, setTarget] = createSignal<Omit<DraftComment, "body"> | null>(null);
  const [draftText, setDraftText] = createSignal("");
  const [summary, setSummary] = createSignal("");
  const [comment, setComment] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const file = (): ReviewFile | null => data()?.files.find((f) => f.path === pane()) ?? null;
  const fileComments = (path: string) =>
    (data()?.reviewComments ?? []).filter((c) => c.path === path && c.line !== null).sort((a, b) => (a.line ?? 0) - (b.line ?? 0));
  const noted = (line: DiffLine) => {
    const f = file();
    const spot = f && lineTarget(f.path, line);
    if (!spot) return false;
    return (
      drafts().some((d) => sameSpot(d, spot)) ||
      fileComments(spot.path).some((c) => c.line === spot.line && (c.side ?? "RIGHT") === spot.side)
    );
  };

  const addDraft = () => {
    const spot = target();
    const body = draftText().trim();
    if (!spot || !body) return;
    setDrafts((d) => [...d, { ...spot, body }]);
    setTarget(null);
    setDraftText("");
  };

  const submit = async (event: ReviewEvent) => {
    const repo = store.repo;
    if (!repo) return;
    if (event === "REQUEST_CHANGES" && !summary().trim() && !drafts().length) {
      notify("error", "Say what needs to change, in the summary or a line comment.");
      return;
    }
    setBusy(true);
    try {
      await api.submitPullRequestReview(repo.id, p.number, event, summary(), drafts());
      notify("success", `${EVENT_LABELS[event]} on #${p.number}`);
      setDrafts([]);
      setSummary("");
      refetch();
    } catch (e) {
      reportError(e, "Could not submit the review");
    } finally {
      setBusy(false);
    }
  };

  const postComment = async () => {
    const repo = store.repo;
    const body = comment().trim();
    if (!repo || !body) return;
    setBusy(true);
    try {
      await api.commentOnPullRequest(repo.id, p.number, body);
      setComment("");
      refetch();
    } catch (e) {
      reportError(e, "Could not post the comment");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={`#${p.number} ${p.title}`}
      onClose={closeDialog}
      class="fill"
      footer={
        <>
          <input
            class="input grow"
            placeholder={drafts().length ? `Review summary (${drafts().length} line comment${drafts().length === 1 ? "" : "s"} pending)` : "Review summary (optional)"}
            value={summary()}
            onInput={(e) => setSummary(e.currentTarget.value)}
          />
          <button class="btn" disabled={busy() || (!summary().trim() && !drafts().length)} onClick={() => submit("COMMENT")}>
            Comment
          </button>
          <button class="btn danger" disabled={busy()} onClick={() => submit("REQUEST_CHANGES")}>
            Request changes
          </button>
          <button class="btn primary" disabled={busy()} onClick={() => submit("APPROVE")}>
            Approve
          </button>
        </>
      }
    >
      <Show when={data()} fallback={data.error ? <div class="empty muted">Could not load this pull request</div> : <Loading label="Loading pull request" />}>
        {(d) => (
          <div class="commit-split" style={{ flex: "1", "min-height": "0", "grid-template-columns": "280px 1fr" }}>
            <div class="list">
              <div class="row" classList={{ selected: pane() === null }} onMouseDown={() => setPane(null)}>
                <span class="grow">Conversation</span>
                <span class="muted small">{d().comments.length}</span>
              </div>
              <For each={d().files}>
                {(f) => (
                  <div class="row" classList={{ selected: pane() === f.path }} onMouseDown={() => setPane(f.path)} title={f.path}>
                    <span class="path grow">
                      <bdi>{f.oldPath ? `${f.oldPath} -> ${f.path}` : f.path}</bdi>
                    </span>
                    <span class="muted small">
                      +{f.diff.additions} -{f.diff.deletions}
                    </span>
                  </div>
                )}
              </For>
            </div>
            <div class="content review-pane">
              <Switch>
                <Match when={pane() === null}>
                  <div class="review-conversation">
                    <div class="review-entry">
                      <div class="muted small">{d().detail.user.login} opened this pull request</div>
                      <Show when={d().detail.body?.trim()} fallback={<p class="muted">No description.</p>}>
                        <Markdown source={d().detail.body!} />
                      </Show>
                    </div>
                    <For each={d().reviews.filter((r) => r.state !== "PENDING")}>
                      {(r) => (
                        <div class="review-entry">
                          <div class="muted small">
                            {r.user.login} {REVIEW_STATES[r.state] ?? r.state.toLowerCase()}
                            {r.submitted_at ? ` ${relativeTime(Date.parse(r.submitted_at) / 1000)}` : ""}
                          </div>
                          <Show when={r.body?.trim()}>
                            <Markdown source={r.body!} />
                          </Show>
                        </div>
                      )}
                    </For>
                    <For each={d().comments}>
                      {(c) => (
                        <div class="review-entry">
                          <div class="muted small">
                            {c.user.login} {relativeTime(Date.parse(c.created_at) / 1000)}
                          </div>
                          <Markdown source={c.body} />
                        </div>
                      )}
                    </For>
                    <textarea class="textarea" placeholder="Leave a comment" value={comment()} onInput={(e) => setComment(e.currentTarget.value)} />
                    <div>
                      <button class="btn" disabled={busy() || !comment().trim()} onClick={postComment}>
                        Comment
                      </button>
                    </div>
                  </div>
                </Match>
                <Match when={file()}>
                  {(f) => (
                    <>
                      <div class="review-diff">
                        <DiffView
                          diff={f().diff}
                          noted={noted}
                          onLineClick={(line) => {
                            const spot = lineTarget(f().path, line);
                            if (spot) setTarget(spot);
                          }}
                        />
                      </div>
                      <div class="review-notes">
                        <Show when={target()}>
                          {(t) => (
                            <div class="review-draft">
                              <span class="muted small">
                                Comment on line {t().line}
                                {t().side === "LEFT" ? " (removed)" : ""}
                              </span>
                              <textarea
                                class="textarea"
                                autofocus
                                value={draftText()}
                                onInput={(e) => setDraftText(e.currentTarget.value)}
                                onKeyDown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && addDraft()}
                              />
                              <div class="summary-row">
                                <button class="btn primary" disabled={!draftText().trim()} onClick={addDraft}>
                                  Add to review
                                </button>
                                <button class="btn" onClick={() => setTarget(null)}>
                                  Cancel
                                </button>
                              </div>
                            </div>
                          )}
                        </Show>
                        <For each={drafts().filter((x) => x.path === f().path)}>
                          {(x) => (
                            <div class="review-entry pending">
                              <div class="muted small">
                                Line {x.line} · pending{" "}
                                <a onClick={() => setDrafts((all) => all.filter((y) => y !== x))}>remove</a>
                              </div>
                              <Markdown source={x.body} />
                            </div>
                          )}
                        </For>
                        <For each={fileComments(f().path)}>
                          {(c) => (
                            <div class="review-entry">
                              <div class="muted small">
                                Line {c.line} · {c.user.login} {relativeTime(Date.parse(c.created_at) / 1000)}
                              </div>
                              <Markdown source={c.body} />
                            </div>
                          )}
                        </For>
                        <Show when={!target() && !fileComments(f().path).length && !drafts().some((x) => x.path === f().path)}>
                          <span class="muted small">Click a line number to comment on that line.</span>
                        </Show>
                      </div>
                    </>
                  )}
                </Match>
              </Switch>
            </div>
          </div>
        )}
      </Show>
    </Dialog>
  );
}
