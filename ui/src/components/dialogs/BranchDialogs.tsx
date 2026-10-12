import { ask } from "@tauri-apps/plugin-dialog";
import { createResource, createSignal, For, Show } from "solid-js";
import { api } from "../../api";
import { currentBranch, notify, run, setStore, store } from "../../store";
import { Dialog } from "../Dialog";
import { BranchPicker, closeDialog, sanitizeBranchName } from "./common";

export function NewBranchDialog(p: { start?: string }) {
  const [name, setName] = createSignal("");
  const clean = () => sanitizeBranchName(name());
  const create = async () => {
    const repo = store.repo;
    if (!repo || !clean()) return;
    const ok = await run("Create branch", () => api.createBranch(repo.id, clean(), p.start).then(() => true));
    if (ok) {
      closeDialog();
      setStore("popover", null);
    }
  };
  return (
    <Dialog
      title="Create a branch"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!clean()} onClick={create}>
            Create branch
          </button>
        </>
      }
    >
      <div class="field">
        <span class="field-label">Name</span>
        <input class="input" autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && create()} />
        <Show when={name() && clean() !== name()}>
          <span class="hint">Will be created as {clean()}</span>
        </Show>
      </div>
      <span class="muted">
        {p.start ? `Your new branch will start at commit ${p.start.slice(0, 7)}.` : `Your new branch will be based on ${currentBranch() ?? "HEAD"}.`}
      </span>
    </Dialog>
  );
}

export function RenameBranchDialog(p: { name: string }) {
  const [name, setName] = createSignal(p.name);
  const clean = () => sanitizeBranchName(name());
  const rename = async () => {
    const repo = store.repo;
    if (!repo) return;
    const ok = await run("Rename branch", () => api.renameBranch(repo.id, p.name, clean()).then(() => true));
    if (ok) closeDialog();
  };
  return (
    <Dialog
      title={`Rename ${p.name}`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!clean() || clean() === p.name} onClick={rename}>
            Rename
          </button>
        </>
      }
    >
      <input class="input" autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && rename()} />
    </Dialog>
  );
}

export function MergeDialog() {
  const [picked, setPicked] = createSignal<string | null>(null);
  const merge = async (squash: boolean) => {
    const repo = store.repo;
    const branch = picked();
    if (!repo || !branch) return;
    closeDialog();
    const result = await run("Merge", () => api.merge(repo.id, branch, squash));
    if (result?.conflicted) notify("error", `There are conflicts merging ${branch}. Resolve them in the Changes tab.`);
    else if (result) notify("success", `Merged ${branch} into ${currentBranch()}`);
  };
  return (
    <Dialog
      title={`Merge into ${currentBranch()}`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" disabled={!picked()} onClick={() => merge(true)}>
            Squash and merge
          </button>
          <button class="btn primary" disabled={!picked()} onClick={() => merge(false)}>
            Create a merge commit
          </button>
        </>
      }
    >
      <BranchPicker value={picked()} onChange={setPicked} excludeCurrent />
    </Dialog>
  );
}

export function RebaseDialog() {
  const [picked, setPicked] = createSignal<string | null>(null);
  const [preview] = createResource(
    () => (store.repo && picked() ? { id: store.repo.id, other: picked()! } : null),
    ({ id, other }) => api.compare(id, other),
  );
  const rebase = async () => {
    const repo = store.repo;
    const onto = picked();
    if (!repo || !onto) return;
    closeDialog();
    const ok = await run("Rebase", () => api.rebase(repo.id, onto).then(() => true));
    if (ok) {
      notify("success", `Rebased ${currentBranch()} onto ${onto}`, {
        action: { label: "Undo", run: () => run("Undo", () => api.undo(repo.id).then(() => undefined)) },
      });
    }
  };
  return (
    <Dialog
      title={`Rebase ${currentBranch()}`}
      onClose={closeDialog}
      footer={
        <>
          <span class="muted small" style={{ "margin-right": "auto" }}>
            <Show when={preview()}>
              {(pv) => `${pv().ahead.length} commit${pv().ahead.length === 1 ? "" : "s"} replayed on ${pv().behind.length} new commit${pv().behind.length === 1 ? "" : "s"}`}
            </Show>
          </span>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!picked()} onClick={rebase}>
            Rebase
          </button>
        </>
      }
    >
      <BranchPicker value={picked()} onChange={setPicked} excludeCurrent />
    </Dialog>
  );
}

export function CherryPickDialog(p: { shas: string[] }) {
  const [picked, setPicked] = createSignal<string | null>(null);
  const pick = async () => {
    const repo = store.repo;
    const target = picked();
    if (!repo || !target) return;
    if (repo.files.length) {
      await ask("Commit or stash your changes before cherry-picking to another branch.", { title: "Uncommitted changes", kind: "warning" });
      return;
    }
    closeDialog();
    const ok = await run("Cherry-pick", () => api.cherryPick(repo.id, p.shas, target === currentBranch() ? undefined : target).then(() => true));
    if (ok) {
      notify("success", `Copied ${p.shas.length} commit${p.shas.length === 1 ? "" : "s"} to ${target}`, {
        action: { label: "Undo", run: () => run("Undo", () => api.undo(repo.id).then(() => undefined)) },
      });
    }
  };
  return (
    <Dialog
      title={`Cherry-pick ${p.shas.length} commit${p.shas.length === 1 ? "" : "s"}`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!picked()} onClick={pick}>
            Cherry-pick to {picked() ?? "branch"}
          </button>
        </>
      }
    >
      <BranchPicker value={picked()} onChange={setPicked} localOnly />
    </Dialog>
  );
}

export function StashesDialog() {
  const [stashes, { refetch }] = createResource(() => store.repo?.id, (id) => api.stashList(id));
  const [message, setMessage] = createSignal("");
  const label = (m: string) => m.replace(/^On [^:]+: /, "").replace(/^!!Blink<(.+)>$/, "Stashed changes on $1");
  const id = () => store.repo!.id;
  const stash = async () => {
    await run("Stash", () => api.stash(id(), message().trim() || undefined));
    setMessage("");
    refetch();
  };
  const discard = async (reference: string) => {
    const ok = await ask("Discard this stash? This cannot be undone.", { title: "Discard stash", kind: "warning", okLabel: "Discard" });
    if (!ok) return;
    await run("Discard stash", () => api.stashDrop(id(), reference));
    refetch();
  };
  return (
    <Dialog title="Stashes" onClose={closeDialog} wide>
      <div class="summary-row">
        <input class="input" placeholder="Stash message (optional)" value={message()} onInput={(e) => setMessage(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && stash()} />
        <button class="btn primary" disabled={!store.repo?.files.length} onClick={stash}>
          Stash all changes
        </button>
      </div>
      <Show when={stashes()?.length} fallback={<div class="empty muted">No stashed changes</div>}>
        <For each={stashes()}>
          {(s) => (
            <div class="static-row">
              <span class="grow">{label(s.message)}</span>
              <span class="muted small">{s.reference}</span>
              <button class="btn" onClick={async () => { await run("Restore stash", () => api.stashPop(id(), s.reference)); refetch(); }}>
                Restore
              </button>
              <button class="btn danger" onClick={() => discard(s.reference)}>
                Discard
              </button>
            </div>
          )}
        </For>
      </Show>
    </Dialog>
  );
}
