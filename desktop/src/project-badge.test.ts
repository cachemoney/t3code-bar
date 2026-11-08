import { expect, it } from 'vitest';
import { projectBadge, projectColor, projectMonogram } from './project-badge';
it('matches Swift Unicode monograms and canonical color hashing', () => {
  expect(['in2', 'T3-Code-Bar', 'single', 'two words', '', '🌸', 'éclair', 'e\u0301clair'].map(projectMonogram)).toEqual(['I2', 'T3', 'SE', 'TW', 'PR', 'PR', 'ÉR', 'ÉR']);
  expect(projectColor('éclair')).toBe(projectColor(' e\u0301CLAIR '));
  expect(projectBadge('No project', null)).toEqual({ kind: 'no-project', text: '', color: '#9ca3af' });
});
it('uses validated custom icon payloads and falls back on malformed ones', () => {
  expect(projectBadge('Test', '{"kind":"monogram","text":"XY","color":"red"}')).toEqual({ kind: 'monogram', text: 'XY', color: '#f87171' });
  expect(projectBadge('Test', '{"kind":"emoji","emoji":"🌱"}').text).toBe('🌱');
  expect(projectBadge('Test', '{"kind":3}')).toEqual(projectBadge('Test', null));
  expect(projectBadge('Test', 'invalid')).toEqual(projectBadge('Test', null));
});
