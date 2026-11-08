import { useLayoutEffect, useRef, useState } from 'react';

export function truncateMiddle(text: string, width: number, measure: (text: string) => number): string {
  if (measure(text) <= width) return text;
  const characters = Array.from(text);
  let low = 0, high = characters.length;
  const candidate = (count: number) => characters.slice(0, Math.ceil(count / 2)).join('') + '…' + characters.slice(characters.length - Math.floor(count / 2)).join('');
  while (low < high) {
    const count = Math.ceil((low + high) / 2);
    if (measure(candidate(count)) <= width) low = count;
    else high = count - 1;
  }
  return candidate(low);
}

export function MiddleText({ text }: { text: string }) {
  const element = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(text);
  useLayoutEffect(() => {
    const span = element.current!;
    const context = document.createElement('canvas').getContext('2d')!;
    const update = () => {
      context.font = getComputedStyle(span).font;
      setVisible(truncateMiddle(text, span.clientWidth, value => context.measureText(value).width));
    };
    const observer = new ResizeObserver(update);
    observer.observe(span);
    update();
    return () => observer.disconnect();
  }, [text]);
  return <span ref={element} className="usage-model-name" title={text}><span aria-hidden="true">{visible}</span><span className="sr-only">{text}</span></span>;
}
