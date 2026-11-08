import { describe, expect, it } from 'vitest';
import { chartModel, curves, nearestColumn, scale } from './chart';
import type { UsageReport } from './contracts';
import { resolveProvider, statusIcons, uniqueProviders } from './providers';

describe('Swift chart parity', () => {
  it('matches the Swift UsageChartModel oracle for steep and turning segments', () => {
    const result = curves([{ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 100 }, { x: 3, y: 20 }]);
    const oracle = [[0, 0, 1/3, 0.01999600119960014, 2/3, 0.00019994001999290578, 1, 1], [1, 1, 4/3, 1.9998000599800072, 5/3, 100, 2, 100], [2, 100, 7/3, 100, 8/3, 46.66666666666667, 3, 20]];
    result.forEach((c, i) => [c.from.x, c.from.y, c.control1.x, c.control1.y, c.control2.x, c.control2.y, c.to.x, c.to.y].forEach((n, j) => expect(n).toBeCloseTo(oracle[i][j], 10)));
    expect(scale(7)).toEqual({ maximum: 8, ticks: [0, 2, 4, 6, 8] });
  });
  it('uses the provider maximum and nearest column, with local hourly titles', () => {
    const report = { resolution: 'hour', timeZone: 'Asia/Ho_Chi_Minh', untilMs: Date.parse('2026-11-06T09:08:00Z'), providers: [{ kind: 'codex' }, { kind: 'claude' }], timeline: [
      { day: 20762, hourStartMs: Date.parse('2026-11-05T09:00:00Z'), costUsd: 8, processedTokens: 0, providers: [{ kind: 'codex', costUsd: 3, processedTokens: 0 }, { kind: 'claude', costUsd: 5, processedTokens: 0 }] },
      { day: 20763, hourStartMs: Date.parse('2026-11-06T08:00:00Z'), costUsd: 7, processedTokens: 0, providers: [{ kind: 'codex', costUsd: 1, processedTokens: 0 }, { kind: 'claude', costUsd: 6, processedTokens: 0 }] },
    ] } as UsageReport;
    const model = chartModel(report, 'cost');
    expect(model.ticks.at(-1)?.label).toBe('$6.00');
    expect(model.series.map(s => s.provider)).toEqual(['claude', 'codex']);
    expect(model.columns.map(c => c.title)).toEqual(['4 PM yesterday', '3 PM today']);
    expect([nearestColumn(-1, 5), nearestColumn(.26, 5), nearestColumn(.5, 5), nearestColumn(2, 5), nearestColumn(0, 0)]).toEqual([0, 1, 2, 4, null]);
  });
});
it('resolves and deduplicates provider drivers and maps status icons', () => {
  expect(uniqueProviders(['codex_a', 'gpt-5', 'claudeAgent_x', 'opus', 'kiro_x', 'unknown', 'another'])).toEqual(['openai', 'claude', 'kiro', 'unknown']);
  expect(uniqueProviders([])).toEqual(['openai']);
  expect(resolveProvider('gemini-2')).toBe('gemini');
  expect(statusIcons.Goal).toBe('circle-dashed');
  expect(statusIcons.Limited).toBe('circle-alert');
  expect(statusIcons.Approval).toBe('shield-question-mark');
});
