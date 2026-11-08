import type { ModelUsage, ProviderUsage, UsageReport } from './contracts';
import { compact, currency, percent, providerStyles } from './format';

export type UsageMetric = 'cost' | 'tokens';
export interface Slice { label: string; amount: number; opacity: number; text: string }
export const sessions = (count: number) => `${count.toLocaleString('en-US')} ${count === 1 ? 'session' : 'sessions'}`;
const usageKinds = new Set(['codex', 'claude', 'grok', 'cursor', 'opencode', 'antigravity', 'pi', 'muse']);
export const usageProviderLabel = (kind: string) => usageKinds.has(kind) ? providerStyles[kind].label : kind;
export const usageProviderIcon = (kind: string) => usageKinds.has(kind) ? kind : '?';
export const costProviderDetail = (p: ProviderUsage, report: UsageReport) => `${percent(report.costUsd === 0 ? 0 : p.costUsd / report.costUsd)} of cost · ${compact(p.processedTokens)} tokens`;
export const tokenProviderDetail = (p: ProviderUsage, report: UsageReport) => `${percent(report.processedTokens > 0 ? p.processedTokens / report.processedTokens : 0)} · ${currency(p.costUsd)}`;
export const unpricedNote = (report: UsageReport) => report.records > 0 && report.unpricedRecords > 0 ? `API estimate excludes ${percent(report.unpricedRecords / report.records)} unpriced records.` : null;

export function slices(report: UsageReport, kind: 'tokens' | 'cost' | 'speed'): Slice[] {
  const t = report.tokens, c = report.costByType, s = report.costBySpeed;
  const entries: [string, number, number][] = kind === 'tokens'
    ? [['Input', t.uncachedInput, .6], ['Cache read', t.cachedInput, .3], ['Cache write', t.cacheCreation, .72], ['Output', t.output, 1]]
    : kind === 'cost'
      ? [['Input', c.input, .6], ['Cache read', c.cacheRead, .3], ['Cache write', c.cacheWrite, .72], ['Output', c.output, 1], ['Other', c.other >= .005 ? c.other : 0, .44]]
      : (s.fast + s.ultrafast > 0 ? [['Standard', s.standard, .34], ['Fast', s.fast, .66], ['Ultrafast', s.ultrafast, 1]] : []);
  return entries.filter(([, amount]) => amount > 0).map(([label, amount, opacity]) => ({ label, amount, opacity, text: kind === 'tokens' ? compact(amount) : currency(amount) }));
}

export function modelRows(report: UsageReport, metric: UsageMetric) {
  const ordered = metric === 'cost' ? report.models : [...report.models].sort((a, b) => b.processedTokens - a.processedTokens || b.costUsd - a.costUsd);
  const value = (m: ModelUsage) => metric === 'cost' ? m.costUsd : m.processedTokens;
  const largest = Math.max(0, ...ordered.map(value));
  return ordered.map((model, index) => {
    const unpriced = model.records > 0 && model.unpricedRecords >= model.records;
    const total = metric === 'cost' ? report.costUsd : report.processedTokens;
    const share = metric === 'cost' && unpriced ? null : total === 0 ? 0 : value(model) / total;
    return { ...model, rank: index + 1, unpriced, costText: unpriced ? 'Unpriced' : currency(model.costUsd), shareText: share === null ? '' : percent(share), tokensText: compact(model.processedTokens), barShare: value(model) > 0 && largest > 0 ? value(model) / largest : null };
  });
}

export function footnotes(report: UsageReport, now: number): string[] {
  const partial = [report.malformedRecords > 0 ? `${report.malformedRecords} malformed or inconsistent usage record(s) were skipped; usage may be incomplete.` : '', report.rejectedFiles > 0 ? `${report.rejectedFiles} cache entries could not be read; usage may be incomplete.` : ''].filter(Boolean).join(' ');
  const result = partial ? [partial] : [];
  if (report.localSourcesPartial) result.push('Some local usage sources could not be read.');
  if (!report.scanCacheMissing && report.cacheModified) {
    const seconds = Math.trunc((now - Date.parse(report.cacheModified)) / 1000);
    result.push(seconds < 60 ? 'Updated just now' : seconds < 3600 ? `Updated ${Math.trunc(seconds / 60)} min ago` : seconds < 86400 ? `Updated ${Math.trunc(seconds / 3600)} h ago` : `Updated ${Math.trunc(seconds / 86400)} d ago`);
  }
  return result;
}

export const visibleModels = <T,>(rows: T[], expanded: boolean): T[] => expanded ? rows : rows.slice(0, 5);
export const modelToggleTitle = (count: number, expanded: boolean) => expanded ? 'Show fewer' : `Show all ${count} models`;
