import type { ThreadStatus } from './contracts';
export function resolveProvider(raw: string): string {
  const name = raw.toLowerCase();
  const drivers: [string, string][] = [['codex', 'openai'], ['claudeagent', 'claude'], ['cursor', 'cursor'], ['grok', 'grok'], ['opencode', 'opencode'], ['antigravity', 'antigravity'], ['fx', 'fx'], ['kiro', 'kiro'], ['pi', 'pi'], ['muse', 'muse']];
  for (const [prefix, kind] of drivers) if (name === prefix || name.startsWith(prefix + '_')) return kind;
  if (name.includes('grok') || name.includes('xai')) return 'grok';
  if (['claude', 'anthropic', 'sonnet', 'opus', 'haiku'].some(s => name.includes(s))) return 'claude';
  if (name.includes('gemini') || name.includes('google')) return 'gemini';
  if (['gpt', 'openai', 'codex'].some(s => name.includes(s)) || name.startsWith('o3') || name.startsWith('o4')) return 'openai';
  return 'unknown';
}
export function uniqueProviders(raws: string[]) { return raws.length ? [...new Set(raws.map(resolveProvider))] : ['openai']; }
export const statusIcons: Record<Exclude<ThreadStatus, 'Settled'>, string> = { Working: 'circle-dashed', Goal: 'circle-dashed', Waiting: 'circle-dot-dashed', Approval: 'shield-question-mark', Input: 'message-circle-question-mark', Request: 'user-round', Failed: 'circle-alert', Limited: 'circle-alert', Woke: 'alarm-clock', Done: 'circle-check' };
