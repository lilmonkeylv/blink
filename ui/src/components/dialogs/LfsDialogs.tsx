import { createSignal } from "solid-js";
import { api } from "../../api";
import { lfsPatternFor } from "../../lib/lfs";
import { notify, refreshLfs, run, store } from "../../store";
import { Field } from "../Controls";
import { Dialog } from "../Dialog";
import { closeDialog } from "./common";

export function LfsTrackDialog(p: { path: string }) {
  const [pattern, setPattern] = createSignal(lfsPatternFor(p.path));
  const clean = () => pattern().trim();
  const valid = () => clean().length > 0 && !clean().startsWith("#") && !/[\r\n]/.test(clean());
  const track = async () => {
    const repo = store.repo;
    if (!repo || !valid()) return;
    const value = clean();
    const done = await run("Track with LFS", () => api.lfsTrack(repo.id, value).then(() => true));
    if (!done) return;
    notify("success", `Tracking ${value} with Git LFS. Commit .gitattributes to share it.`);
    closeDialog();
    void refreshLfs();
  };
  return (
    <Dialog
      title="Track with Git LFS"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!valid()} onClick={track}>
            Track
          </button>
        </>
      }
    >
      <Field
        label="Pattern"
        hint="Added to .gitattributes. Matching files are stored in Git LFS from their next commit; versions already in history stay in Git."
      >
        <input class="input" autofocus value={pattern()} onInput={(e) => setPattern(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && track()} />
      </Field>
    </Dialog>
  );
}
