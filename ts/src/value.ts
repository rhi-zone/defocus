/** The universal value type. Mirrors Marinada's value model. */
export type Value =
  | null
  | boolean
  | number
  | string
  | Value[]
  | { [key: string]: Value };

/** Expressions are Values — an array with a string first element is a call. */
export type Expr = Value;

export type Identity = string;

export function isTruthy(v: Value): boolean {
  if (v === null) return false;
  if (typeof v === "boolean") return v;
  if (typeof v === "number") return v !== 0;
  if (typeof v === "string") return v.length > 0;
  if (Array.isArray(v)) return v.length > 0;
  return Object.keys(v).length > 0;
}

export function asStr(v: Value): string | undefined {
  return typeof v === "string" ? v : undefined;
}

export function asNum(v: Value): number | undefined {
  return typeof v === "number" ? v : undefined;
}

export function asArray(v: Value): Value[] | undefined {
  return Array.isArray(v) ? v : undefined;
}

export function asRecord(v: Value): { [key: string]: Value } | undefined {
  if (v !== null && typeof v === "object" && !Array.isArray(v)) return v;
  return undefined;
}

export function getIn(v: Value, ...path: string[]): Value {
  let current = v;
  for (const key of path) {
    const rec = asRecord(current);
    if (!rec) return null;
    current = rec[key] ?? null;
  }
  return current;
}
