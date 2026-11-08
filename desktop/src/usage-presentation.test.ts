import { describe, expect, it } from 'vitest';
import type { UsageReport } from './contracts';
import { costProviderDetail, footnotes, modelRows, sessions, slices, tokenProviderDetail, unpricedNote, usageProviderLabel, visibleModels, modelToggleTitle } from './usage-presentation';

const report: UsageReport = {
  costUsd: 10, processedTokens: 10000, sessions: 1, records: 10, unpricedRecords: 1, duplicatesDropped: 0, cacheSavingsUsd: 2,
  tokens: { uncachedInput: 1000, cachedInput: 2000, cacheCreation: 0, output: 7000, reasoning: 0 },
  costByType: { input: 1, cacheRead: 0, cacheWrite: 2, output: 7, other: .004 },
  costBySpeed: { standard: 8, fast: 2, ultrafast: 0, premium: .4 },
  providers: [{ kind: 'claude', label: 'Claude', costUsd: 2, processedTokens: 1000, sessions: 1, records: 2 }],
  models: [{ kind: 'codex', name: 'cost-first', costUsd: 8, processedTokens: 2000, records: 2, unpricedRecords: 0 }, { kind: 'muse', name: 'tokens-first', costUsd: 0, processedTokens: 8000, records: 1, unpricedRecords: 1 }],
  timeline: [], resolution: 'day', timeZone: 'UTC', untilMs: 0, notices: [], malformedRecords: 2, rejectedFiles: 1, cacheModified: '2026-11-07T00:00:00Z', scanCacheMissing: false, localSourcesPartial: true,
};

describe('Swift Usage presentation', () => {
  it('uses canonical labels and provider details', () => {
    expect(['claude', 'grok', 'muse', 'future'].map(usageProviderLabel)).toEqual(['Claude Code', 'Grok Build', 'Muse Code', 'future']);
    expect(sessions(1)).toBe('1 session');
    expect(sessions(1234)).toBe('1,234 sessions');
    expect(costProviderDetail(report.providers[0], report)).toBe('20.0% of cost · 1K tokens');
    expect(tokenProviderDetail(report.providers[0], report)).toBe('10.0% · $2.00');
    expect(unpricedNote(report)).toBe('API estimate excludes 10.0% unpriced records.');
  });
  it('keeps slice order, zero filtering, threshold and speed colors', () => {
    expect(slices(report, 'cost').map(s => [s.label, s.opacity, s.text])).toEqual([['Input', .6, '$1.00'], ['Cache write', .72, '$2.00'], ['Output', 1, '$7.00']]);
    expect(slices(report, 'tokens').map(s => s.label)).toEqual(['Input', 'Cache read', 'Output']);
    expect(slices(report, 'speed').map(s => [s.label, s.opacity])).toEqual([['Standard', .34], ['Fast', .66]]);
    expect(slices({ ...report, costBySpeed: { standard: 8, fast: 0, ultrafast: 0, premium: 0 } }, 'speed')).toEqual([]);
  });
  it('ranks models by the selected metric and preserves unpriced behavior', () => {
    expect(modelRows(report, 'cost').map(r => [r.name, r.costText, r.shareText, r.barShare])).toEqual([['cost-first', '$8.00', '80.0%', 1], ['tokens-first', 'Unpriced', '', null]]);
    expect(modelRows(report, 'tokens').map(r => [r.name, r.shareText, r.barShare])).toEqual([['tokens-first', '80.0%', 1], ['cost-first', '20.0%', .25]]);
  });
  it('collapses at five models and names the toggle', () => {
    expect(visibleModels([1, 2, 3, 4, 5, 6], false)).toEqual([1, 2, 3, 4, 5]);
    expect(visibleModels([1, 2, 3, 4, 5, 6], true)).toHaveLength(6);
    expect(modelToggleTitle(6, false)).toBe('Show all 6 models');
    expect(modelToggleTitle(6, true)).toBe('Show fewer');
  });
  it('reports partial sources and cache age using Swift wording', () => {
    expect(footnotes(report, Date.parse('2026-11-07T01:01:00Z'))).toEqual(['2 malformed or inconsistent usage record(s) were skipped; usage may be incomplete. 1 cache entries could not be read; usage may be incomplete.', 'Some local usage sources could not be read.', 'Updated 1 h ago']);
  });
});
