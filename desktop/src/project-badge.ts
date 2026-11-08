export const projectPalette = [
  ['gray', '#9ca3af'], ['red', '#f87171'], ['orange', '#fb923c'], ['amber', '#fbbf24'],
  ['yellow', '#facc15'], ['lime', '#a3e635'], ['green', '#4ade80'], ['emerald', '#34d399'],
  ['teal', '#2dd4bf'], ['cyan', '#22d3ee'], ['sky', '#38bdf8'], ['blue', '#60a5fa'],
  ['indigo', '#818cf8'], ['violet', '#a78bfa'], ['purple', '#c084fc'], ['fuchsia', '#e879f9'],
  ['pink', '#f472b6'], ['rose', '#fb7185'],
] as const;

const graphemes = (value: string) => Array.from(new Intl.Segmenter('en', { granularity: 'grapheme' }).segment(value), part => part.segment);
export function projectMonogram(title: string): string {
  const words = title.normalize('NFC').trim().match(/[\p{L}\p{N}]+/gu) ?? [];
  if (!words.length) return 'PR';
  const first = graphemes(words[0]!);
  const second = first.slice(1).find(character => /\p{N}/u.test(character))
    ?? (words.length > 1 ? graphemes(words[words.length - 1]!)[0] : first[first.length - 1]) ?? first[0];
  return graphemes((first[0] + second).toUpperCase()).slice(0, 2).join('');
}
export function projectColor(title: string): string {
  const value = title.normalize('NFC').trim().toLocaleLowerCase('en-US') || 'project';
  let hash = 0;
  for (const scalar of value) hash = (hash * 31 + scalar.codePointAt(0)!) % projectPalette.length;
  return projectPalette[hash][1];
}
export interface ProjectBadgeData { kind: 'monogram' | 'emoji' | 'folder' | 'no-project'; text: string; color: string }
export function projectBadge(title: string, raw: string | null): ProjectBadgeData {
  let icon: Record<string, unknown> | null = null;
  if (raw) {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        && ['kind', 'text', 'color', 'emoji', 'name'].every(key => !(key in parsed) || (parsed as Record<string, unknown>)[key] == null || typeof (parsed as Record<string, unknown>)[key] === 'string')) icon = parsed as Record<string, unknown>;
    } catch { icon = null; }
  }
  const named = (value: unknown) => projectPalette.find(([name]) => name === (typeof value === 'string' ? value.trim().toLowerCase() : ''))?.[1] ?? '#60a5fa';
  if (['no project', 'noproject'].includes(title.trim().toLowerCase()) || icon?.name === 'message-square-dashed') {
    return { kind: 'no-project', text: '', color: named(icon?.color ?? 'gray') };
  }
  if (icon?.kind === 'monogram') return { kind: 'monogram', text: typeof icon.text === 'string' ? icon.text : projectMonogram(title), color: named(icon.color) };
  if (icon?.kind === 'emoji' && typeof icon.emoji === 'string' && icon.emoji) return { kind: 'emoji', text: icon.emoji, color: 'inherit' };
  if (icon?.kind === 'lucide') return { kind: 'folder', text: '', color: named(icon.color) };
  return { kind: 'monogram', text: projectMonogram(title), color: projectColor(title) };
}
