import { Match, Show, Switch } from "solid-js";
import { store, type Dialog } from "../store";
import { CherryPickDialog, MergeDialog, NewBranchDialog, RebaseDialog, RenameBranchDialog, StashesDialog } from "./dialogs/BranchDialogs";
import {
  BlameDialog,
  FileHistoryDialog,
  InteractiveRebaseDialog,
  ResetDialog,
  SquashDialog,
  TagDialog,
  UndoHistoryDialog,
} from "./dialogs/HistoryDialogs";
import { CloneDialog, CreateRepoDialog, ImportDialog, InitRepoDialog, PublishDialog, RepoSettingsDialog } from "./dialogs/RepoDialogs";
import { ApplyPatchDialog, ChecksDialog, MergeEditorDialog, ShortcutsDialog, SignInDialog, WorktreesDialog } from "./dialogs/ToolDialogs";
import { SettingsDialog } from "./Settings";

function of<T extends Dialog["type"]>(type: T): Extract<Dialog, { type: T }> | false {
  const d = store.dialog;
  return d?.type === type ? (d as Extract<Dialog, { type: T }>) : false;
}

export function Dialogs() {
  return (
    <Show when={store.dialog}>
      <Switch>
        <Match when={of("addRepo")}>{(d) => <InitRepoDialog path={d().initPath ?? ""} />}</Match>
        <Match when={of("createRepo")}>
          <CreateRepoDialog />
        </Match>
        <Match when={of("clone")}>{(d) => <CloneDialog url={d().url} />}</Match>
        <Match when={of("publish")}>
          <PublishDialog />
        </Match>
        <Match when={of("repoSettings")}>
          <RepoSettingsDialog />
        </Match>
        <Match when={of("import")}>
          <ImportDialog />
        </Match>
        <Match when={of("newBranch")}>{(d) => <NewBranchDialog start={d().start} />}</Match>
        <Match when={of("renameBranch")}>{(d) => <RenameBranchDialog name={d().name} />}</Match>
        <Match when={of("merge")}>
          <MergeDialog />
        </Match>
        <Match when={of("rebase")}>
          <RebaseDialog />
        </Match>
        <Match when={of("cherryPick")}>{(d) => <CherryPickDialog shas={d().shas} />}</Match>
        <Match when={of("stashes")}>
          <StashesDialog />
        </Match>
        <Match when={of("tag")}>{(d) => <TagDialog sha={d().sha} />}</Match>
        <Match when={of("reset")}>{(d) => <ResetDialog sha={d().sha} />}</Match>
        <Match when={of("interactiveRebase")}>{(d) => <InteractiveRebaseDialog sha={d().sha} />}</Match>
        <Match when={of("squash")}>{(d) => <SquashDialog target={d().target} commits={d().commits} />}</Match>
        <Match when={of("undoHistory")}>
          <UndoHistoryDialog />
        </Match>
        <Match when={of("fileHistory")}>{(d) => <FileHistoryDialog path={d().path} />}</Match>
        <Match when={of("blame")}>{(d) => <BlameDialog path={d().path} />}</Match>
        <Match when={of("signIn")}>
          <SignInDialog />
        </Match>
        <Match when={of("shortcuts")}>
          <ShortcutsDialog />
        </Match>
        <Match when={of("worktrees")}>
          <WorktreesDialog />
        </Match>
        <Match when={of("applyPatch")}>
          <ApplyPatchDialog />
        </Match>
        <Match when={of("checks")}>
          <ChecksDialog />
        </Match>
        <Match when={of("mergeEditor")}>{(d) => <MergeEditorDialog path={d().path} />}</Match>
        <Match when={of("settings")}>{(d) => <SettingsDialog tab={d().tab} />}</Match>
      </Switch>
    </Show>
  );
}
