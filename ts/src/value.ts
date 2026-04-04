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

export function isRef(v: Value): boolean {
  if (
    v === null ||
    typeof v !== "object" ||
    Array.isArray(v) ||
    !("$ref" in v) ||
    typeof v.$ref !== "string"
  ) return false;
  const keys = Object.keys(v);
  if (keys.length === 1) return true;
  if (keys.length === 2 && "$verbs" in v && Array.isArray(v.$verbs)) return true;
  return false;
}

export function asRef(v: Value): string | undefined {
  if (isRef(v)) return (v as { $ref: string }).$ref;
  return undefined;
}

export function refVerbs(v: Value): string[] | undefined {
  if (!isRef(v)) return undefined;
  const rec = v as { $ref: string; $verbs?: Value };
  if ("$verbs" in rec && Array.isArray(rec.$verbs)) {
    return rec.$verbs.filter((x): x is string => typeof x === "string");
  }
  return undefined;
}

export function isTruthy(v: Value): boolean {
  if (v === null) return false;
  if (typeof v === "boolean") return v;
  if (typeof v === "number") return v !== 0;
  if (typeof v === "string") return v.length > 0;
  if (Array.isArray(v)) return v.length > 0;
  if (isRef(v)) return true;
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
