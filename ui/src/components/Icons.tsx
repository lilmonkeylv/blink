import type { Component, JSX } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { LucideProps } from "lucide-solid";
import Archive from "lucide-solid/icons/archive";
import ArrowDown from "lucide-solid/icons/arrow-down";
import ArrowUp from "lucide-solid/icons/arrow-up";
import Check from "lucide-solid/icons/check";
import ChevronDownGlyph from "lucide-solid/icons/chevron-down";
import ChevronRightGlyph from "lucide-solid/icons/chevron-right";
import CircleAlert from "lucide-solid/icons/circle-alert";
import CircleCheck from "lucide-solid/icons/circle-check";
import Columns2 from "lucide-solid/icons/columns-2";
import Copy from "lucide-solid/icons/copy";
import Ellipsis from "lucide-solid/icons/ellipsis";
import ExternalLink from "lucide-solid/icons/external-link";
import FolderGit from "lucide-solid/icons/folder-git-2";
import FolderOpen from "lucide-solid/icons/folder-open";
import GitBranch from "lucide-solid/icons/git-branch";
import GitPullRequest from "lucide-solid/icons/git-pull-request";
import Info from "lucide-solid/icons/info";
import ListFilter from "lucide-solid/icons/list-filter";
import Lock from "lucide-solid/icons/lock";
import Menu from "lucide-solid/icons/menu";
import Pilcrow from "lucide-solid/icons/pilcrow";
import Plus from "lucide-solid/icons/plus";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import Rows2 from "lucide-solid/icons/rows-2";
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
export const SuccessIcon = icon(CircleCheck);
export const ErrorIcon = icon(CircleAlert);
export const InfoIcon = icon(Info);
export const WhitespaceIcon = icon(Pilcrow);
export const UnifiedIcon = icon(Rows2);
export const SplitIcon = icon(Columns2);
export const MoreIcon = icon(Ellipsis);
export const FilterIcon = icon(ListFilter);
export const CopyIcon = icon(Copy);
export const StashIcon = icon(Archive);
export const ExternalIcon = icon(ExternalLink);
export const FolderOpenIcon = icon(FolderOpen);
export const LockIcon = icon(Lock);

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
