import { describe, expect, it } from 'vitest';
import { compact, currency, duration, percent, relative, until } from './format';

describe('Swift presentation parity', () => {
  it('preserves significant digits and drops only all-zero fractions', () => {
    expect([999, 4200, 1500000, 193000000, 16000000000].map(compact)).toEqual(['999', '4.20K', '1.50M', '193M', '16B']);
    expect(currency(1234.567)).toBe('$1,234.57');
    expect(currency(-12.5)).toBe('-$12.50');
    expect(percent(0.0001)).toBe('<0.1%');
    expect(percent(0)).toBe('0.0%');
  });
  it('uses the same elapsed and rounded-up reset boundaries', () => {
    const now = Date.parse('2026-11-06T09:08:00Z');
    expect(relative('2026-11-06T09:07:01Z', now)).toBe('now');
    expect(duration('2026-11-06T08:07:00Z', now)).toBe('1h 1m');
    expect(until('2026-11-06T10:07:01Z', now)).toBe('1h');
    expect(until('2026-11-06T09:08:00Z', now)).toBeNull();
    expect(relative('invalid', now)).toBe('');
  });
});
