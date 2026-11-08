import type { ChartSlot, UsageReport } from './contracts';
import { compact, currency } from './format';

type Point = { x: number; y: number };
export type Curve = { from: Point; control1: Point; control2: Point; to: Point };
export function scale(maximum: number) {
  if (maximum <= 0) return { maximum: 0, ticks: [0] };
  const raw = maximum / 4, power = 10 ** Math.floor(Math.log10(raw)), ratio = raw / power;
  const step = (ratio > 5 ? 10 : ratio > 2 ? 5 : ratio > 1 ? 2 : 1) * power;
  const top = Math.ceil(maximum / step) * step;
  const ticks: number[] = [];
  for (let value = 0; value <= top + step * 1e-6; value += step) ticks.push(value);
  return { maximum: top, ticks };
}
export function curves(points: Point[]): Curve[] {
  if (points.length < 2) return [];
  const secants = points.slice(1).map((p, i) => (p.x - points[i].x) === 0 ? 0 : (p.y - points[i].y) / (p.x - points[i].x));
  const slopes = points.map((_, i) => i === 0 ? secants[0] : i === points.length - 1 ? secants.at(-1)! : secants[i - 1] * secants[i] <= 0 ? 0 : (secants[i - 1] + secants[i]) / 2);
  secants.forEach((s, i) => {
    if (s === 0) { slopes[i] = 0; slopes[i + 1] = 0; return; }
    const a = slopes[i] / s, b = slopes[i + 1] / s, sum = a * a + b * b;
    if (sum > 9) { const f = 3 / Math.sqrt(sum); slopes[i] = f * a * s; slopes[i + 1] = f * b * s; }
  });
  return points.slice(1).map((to, i) => { const from = points[i], run = to.x - from.x; return { from, control1: { x: from.x + run / 3, y: from.y + slopes[i] * run / 3 }, control2: { x: to.x - run / 3, y: to.y - slopes[i + 1] * run / 3 }, to }; });
}
export function nearestColumn(fraction: number, count: number): number | null {
  return count ? Math.round(Math.min(1, Math.max(0, fraction)) * (count - 1)) : null;
}
function localParts(ms: number, zone: string) {
  const parts = new Intl.DateTimeFormat('en-US', { timeZone: zone, year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', hourCycle: 'h23' }).formatToParts(ms);
  const get = (type: string) => Number(parts.find(p => p.type === type)?.value);
  return { day: Math.floor(Date.UTC(get('year'), get('month') - 1, get('day')) / 86400000), hour: get('hour') };
}
function dayLabel(day: number) { return new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', month: 'short', day: 'numeric' }).format(day * 86400000); }
function clock(hour: number) { return `${hour % 12 || 12} ${hour < 12 ? 'AM' : 'PM'}`; }
export function chartModel(report: UsageReport, metric: 'cost' | 'tokens') {
  const value = (v: { costUsd: number; processedTokens: number }) => metric === 'cost' ? v.costUsd : v.processedTokens;
  const format = metric === 'cost' ? currency : compact;
  const providers = report.providers.map(p => p.kind);
  const values = (slot: ChartSlot, kind: string) => value(slot.providers.find(p => p.kind === kind) ?? { costUsd: 0, processedTokens: 0 });
  const max = Math.max(0, ...report.timeline.flatMap(c => providers.map(p => values(c, p))));
  const domain = scale(max), y = (n: number) => domain.maximum === 0 ? 260 : 260 - n / domain.maximum * 252;
  const series = providers.map((provider, index) => {
    const nums = report.timeline.map(c => values(c, provider));
    return { provider, index, total: nums.reduce((a, b) => a + b, 0), curves: curves(nums.map((n, i) => ({ x: nums.length < 2 ? 0 : i * 960 / (nums.length - 1), y: y(n) }))) };
  }).sort((a, b) => b.total - a.total || a.index - b.index);
  const axis = (slot: ChartSlot) => slot.hourStartMs == null ? dayLabel(slot.day) : clock(localParts(slot.hourStartMs, report.timeZone).hour);
  const title = (slot: ChartSlot) => {
    if (slot.hourStartMs == null) return dayLabel(slot.day);
    const p = localParts(slot.hourStartMs, report.timeZone), delta = localParts(report.untilMs, report.timeZone).day - p.day;
    return delta === 0 ? `${clock(p.hour)} today` : delta === 1 ? `${clock(p.hour)} yesterday` : `${dayLabel(p.day)}, ${clock(p.hour)}`;
  };
  const cols = report.timeline.map(c => ({ title: title(c), rows: providers.map(provider => ({ provider, text: format(values(c, provider)) })), total: format(value(c)) }));
  const n = report.timeline.length;
  return { heading: `${report.resolution === 'hour' ? 'Hourly' : 'Daily'} ${metric === 'cost' ? 'cost' : 'processed tokens'}`, ticks: domain.ticks.map(v => ({ y: y(v), label: v === 0 ? '0' : format(v) })), series, columns: cols, labels: n ? [0, Math.floor(n / 2), n - 1].map(i => axis(report.timeline[i]).toUpperCase()) : [], total: format(report.timeline.reduce((sum, c) => sum + value(c), 0)) };
}
