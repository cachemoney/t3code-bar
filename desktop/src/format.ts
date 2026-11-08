const integer = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });
const money = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' });

export function compact(value: number): string {
  for (const [scale, symbol] of [[1e12, 'T'], [1e9, 'B'], [1e6, 'M'], [1e3, 'K']] as const) {
    if (Math.abs(value) < scale) continue;
    const n = value / scale;
    const digits = Math.abs(n) >= 100 ? 0 : Math.abs(n) >= 10 ? 1 : 2;
    const text = n.toFixed(digits);
    return text.replace(/\.0+$/, '') + symbol;
  }
  return integer.format(Math.floor(value + 0.5));
}
export function currency(value: number): string { return money.format(value); }
export function percent(share: number): string { return share > 0 && share < .001 ? '<0.1%' : `${(share * 100).toFixed(1)}%`; }
export function relative(date: string | null, now = Date.now()): string {
  if (!date) return '';
  const seconds = Math.max(0, Math.floor((now - Date.parse(date)) / 1000));
  if (!Number.isFinite(seconds)) return '';
  if (seconds < 60) return 'now';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}
export function duration(date: string | null, now = Date.now()): string {
  if (!date) return '';
  const seconds = Math.max(0, Math.floor((now - Date.parse(date)) / 1000));
  if (!Number.isFinite(seconds)) return '';
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h ${Math.floor(seconds / 60) % 60}m`;
}
export function until(date: string | null, now = Date.now()): string | null {
  if (!date) return null;
  const minutes = Math.ceil((Date.parse(date) - now) / 60000);
  if (!Number.isFinite(minutes) || minutes <= 0) return null;
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.ceil(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.ceil(hours / 24)}d`;
}
export const providerStyles: Record<string, { label: string; color: string; icon?: string }> = {
  codex: { label: 'Codex', color: '#f4f4f5' },
  claude: { label: 'Claude Code', color: '#d97757' },
  claudeAgent: { label: 'Claude Code', color: '#d97757' },
  grok: { label: 'Grok Build', color: '#b0b0b0', icon: 'grok' },
  cursor: { label: 'Cursor', color: '#8b8b8b', icon: 'cursor' },
  opencode: { label: 'OpenCode', color: '#5b9bbd', icon: 'opencode' },
  antigravity: { label: 'Antigravity', color: '#8c7bd1', icon: 'antigravity' },
  pi: { label: 'Pi', color: '#4d9f8a', icon: 'pi' },
  muse: { label: 'Muse Code', color: '#0866ff', icon: 'muse' },
};
