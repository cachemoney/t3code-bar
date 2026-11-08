import { expect, it } from 'vitest';
import { truncateMiddle } from './middle-text';

it('keeps both model-name ends within the available measured width', () => {
  const measure = (text: string) => Array.from(text).length;
  expect(truncateMiddle('claude-opus-20261101', 10, measure)).toBe('claud…1101');
  expect(truncateMiddle('gpt-5', 10, measure)).toBe('gpt-5');
  expect(truncateMiddle('a😀bc😀d', 5, measure)).toBe('a😀…😀d');
});
