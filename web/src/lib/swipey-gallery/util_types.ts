export type Point = { x: number; y: number };
export type Size = { width: number; height: number };

export function pointsEqual(p1: Point, p2: Point): boolean {
  return p1.x == p2.x && p1.y == p2.y;
}

export function distance(p1: Point, p2: Point): number {
  const dx = Math.abs(p1.x - p2.x);
  const dy = Math.abs(p1.y - p2.y);
  return Math.sqrt(dx * dx + dy * dy);
}
