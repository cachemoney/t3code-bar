export function limitDuration(milliseconds: number): string {
  const minutes = Math.floor(Math.max(0, milliseconds) / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  return days > 0 ? `${days}d ${hours % 24}h` : hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}
export function limitReset(reset: string | null, now = Date.now()): string | null {
  if (!reset) return null;
  const remaining = Date.parse(reset) - now;
  if (!Number.isFinite(remaining)) return null;
  return remaining <= 0 ? 'resets now' : remaining < 60000 ? 'Resets momentarily' : `resets in ${limitDuration(remaining)}`;
}
export function limitAgo(date: string, now = Date.now()): string {
  const age = Math.max(0, now - Date.parse(date));
  if (!Number.isFinite(age) || age < 60000) return 'just now';
  const minutes = Math.floor(age / 60000);
  return minutes < 60 ? `${minutes}m ago` : minutes < 1440 ? `${Math.floor(minutes / 60)}h ago` : `${Math.floor(minutes / 1440)}d ago`;
}
export function limitsStaleness(checkedAt: string | null, now = Date.now()): string | null {
  if (!checkedAt || now - Date.parse(checkedAt) <= 1800000 || !Number.isFinite(Date.parse(checkedAt))) return null;
  return `T3 Code last checked ${limitAgo(checkedAt, now)}. These numbers may be out of date. Open T3 Code to refresh them.`;
}
