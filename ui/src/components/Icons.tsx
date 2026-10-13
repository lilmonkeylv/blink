import type { Component, JSX } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { LucideProps } from "lucide-solid";
import ArrowDown from "lucide-solid/icons/arrow-down";
import ArrowUp from "lucide-solid/icons/arrow-up";
import Check from "lucide-solid/icons/check";
import ChevronDownGlyph from "lucide-solid/icons/chevron-down";
import ChevronRightGlyph from "lucide-solid/icons/chevron-right";
import FolderGit from "lucide-solid/icons/folder-git-2";
import GitBranch from "lucide-solid/icons/git-branch";
import GitPullRequest from "lucide-solid/icons/git-pull-request";
import Menu from "lucide-solid/icons/menu";
import Plus from "lucide-solid/icons/plus";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import SquareArrowRight from "lucide-solid/icons/square-arrow-right";
import SquareDot from "lucide-solid/icons/square-dot";
import SquareMinus from "lucide-solid/icons/square-minus";
import SquarePlus from "lucide-solid/icons/square-plus";
import TriangleAlert from "lucide-solid/icons/triangle-alert";
import X from "lucide-solid/icons/x";
import type { ChangeKind } from "../api";

type P = { class?: string };

const icon = (Glyph: Component<LucideProps>) => (p: P) => <Glyph class={`icon ${p.class ?? ""}`} aria-hidden="true" />;

export const RepoIcon = icon(FolderGit);
export const BranchIcon = icon(GitBranch);
export const SyncIcon = icon(RefreshCw);
export const UpIcon = icon(ArrowUp);
export const DownIcon = icon(ArrowDown);
export const ChevronRight = icon(ChevronRightGlyph);
export const ChevronDown = icon(ChevronDownGlyph);
export const MenuIcon = icon(Menu);
export const PullRequestIcon = icon(GitPullRequest);
export const PlusIcon = icon(Plus);
export const CheckIcon = icon(Check);
export const CloseIcon = icon(X);

const statusGlyphs: Record<ChangeKind, Component<LucideProps>> = {
  added: SquarePlus,
  untracked: SquarePlus,
  modified: SquareDot,
  typechange: SquareDot,
  deleted: SquareMinus,
  renamed: SquareArrowRight,
  copied: SquareArrowRight,
  conflicted: TriangleAlert,
};

export function StatusIcon(p: { kind: ChangeKind }): JSX.Element {
  return <Dynamic component={statusGlyphs[p.kind]} class={`icon status ${p.kind}`} aria-label={p.kind} />;
}
