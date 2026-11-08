import { expect, it } from 'vitest';
import { limitDuration, limitReset, limitsStaleness } from './limits-format';
it('matches Swift limits reset floor and staleness boundaries', () => {
  const now = Date.parse('2026-11-06T09:08:00Z');
  expect(limitDuration(183 * 60000 + 59000)).toBe('3h 3m');
  expect(limitDuration(52 * 3600000)).toBe('2d 4h');
  expect(limitReset('2026-11-06T09:08:59Z', now)).toBe('Resets momentarily');
  expect(limitReset('2026-11-06T09:08:00Z', now)).toBe('resets now');
  expect(limitReset('2026-11-06T10:07:59Z', now)).toBe('resets in 59m');
  expect(limitsStaleness('2026-11-06T08:38:00Z', now)).toBeNull();
  expect(limitsStaleness('2026-11-06T08:37:59Z', now)).toContain('30m ago');
});
