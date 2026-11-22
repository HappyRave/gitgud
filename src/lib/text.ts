/**
 * Shortens `text` to `max` characters by cutting the middle, so both a branch's
 * folder and its ending stay readable (R1 state 5).
 */
export function middleTruncate(text: string, max: number): string {
  if (text.length <= max) {
    return text;
  }
  const keep = max - 1;
  const head = Math.ceil(keep * 0.4);
  return `${text.slice(0, head)}…${text.slice(text.length - (keep - head))}`;
}
