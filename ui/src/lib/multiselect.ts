export interface ClickModifiers {
  shift: boolean;
  toggle: boolean;
}

export function nextSelection(
  order: readonly string[],
  current: ReadonlySet<string>,
  anchor: string | null,
  target: string,
  mods: ClickModifiers,
): { selected: Set<string>; anchor: string } {
  if (mods.shift && anchor !== null && order.includes(anchor)) {
    const [from, to] = [order.indexOf(anchor), order.indexOf(target)].sort((a, b) => a - b);
    const range = order.slice(from, to + 1);
    const selected = mods.toggle ? new Set([...current, ...range]) : new Set(range);
    return { selected, anchor };
  }
  if (mods.toggle) {
    const selected = new Set(current);
    if (selected.has(target) && selected.size > 1) selected.delete(target);
    else selected.add(target);
    return { selected, anchor: target };
  }
  return { selected: new Set([target]), anchor: target };
}
