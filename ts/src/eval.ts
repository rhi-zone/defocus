import type { Value, Expr } from "./value.js";
import { isTruthy, asStr, asNum, asArray, asRecord } from "./value.js";
import type { Effect, Message } from "./world.js";

interface Env {
  bindings: Array<[string, Value]>;
  effects: Effect[];
}

function envNew(): Env {
  return { bindings: [], effects: [] };
}

function envBind(env: Env, name: string, value: Value): void {
  env.bindings.push([name, value]);
}

function envGet(env: Env, name: string): Value {
  for (let i = env.bindings.length - 1; i >= 0; i--) {
    if (env.bindings[i][0] === name) return env.bindings[i][1];
  }
  return null;
}

function envPushScope(env: Env): number {
  return env.bindings.length;
}

function envPopScope(env: Env, mark: number): void {
  env.bindings.length = mark;
}

export function evalHandler(
  handler: Expr,
  payload: Value,
  state: Value,
): Effect[] {
  const env = envNew();
  envBind(env, "payload", payload);
  envBind(env, "state", state);
  evaluate(handler, env);
  return env.effects;
}

function evaluate(expr: Expr, env: Env): Value {
  if (expr === null || typeof expr === "boolean" || typeof expr === "number") {
    return expr;
  }
  if (typeof expr === "string") return expr;
  if (Array.isArray(expr)) {
    if (expr.length === 0) return [];
    const op = asStr(expr[0]);
    if (op === undefined) {
      return expr.map((v) => evaluate(v, env));
    }
    return evalCall(op, expr.slice(1), env);
  }
  // Record — evaluate values
  const result: { [key: string]: Value } = {};
  for (const [k, v] of Object.entries(expr)) {
    result[k] = evaluate(v, env);
  }
  return result;
}

function evalCall(op: string, args: Value[], env: Env): Value {
  switch (op) {
    case "get": {
      const key = evaluate(args[0], env);
      const name = asStr(key);
      return name !== undefined ? envGet(env, name) : null;
    }

    case "get-in": {
      let current = evaluate(args[0], env);
      for (let i = 1; i < args.length; i++) {
        const key = evaluate(args[i], env);
        const rec = asRecord(current);
        const arr = asArray(current);
        if (rec && typeof key === "string") {
          current = rec[key] ?? null;
        } else if (arr && typeof key === "number") {
          current = arr[key] ?? null;
        } else {
          return null;
        }
      }
      return current;
    }

    case "if": {
      const cond = evaluate(args[0], env);
      if (isTruthy(cond)) return evaluate(args[1], env);
      return args.length > 2 ? evaluate(args[2], env) : null;
    }

    case "do": {
      let result: Value = null;
      for (const arg of args) result = evaluate(arg, env);
      return result;
    }

    case "let": {
      const name = asStr(args[0] as Value) ?? "_";
      const value = evaluate(args[1], env);
      const mark = envPushScope(env);
      envBind(env, name, value);
      const result = evaluate(args[2], env);
      envPopScope(env, mark);
      return result;
    }

    case "+":
    case "-":
    case "*":
    case "/":
      return numericBinop(op, args, env);

    case "=":
      return deepEqual(evaluate(args[0], env), evaluate(args[1], env));
    case "!=":
      return !deepEqual(evaluate(args[0], env), evaluate(args[1], env));
    case "<":
      return compareOp(args, env, (o) => o < 0);
    case ">":
      return compareOp(args, env, (o) => o > 0);
    case "<=":
      return compareOp(args, env, (o) => o <= 0);
    case ">=":
      return compareOp(args, env, (o) => o >= 0);

    case "and": {
      const a = evaluate(args[0], env);
      return isTruthy(a) ? evaluate(args[1], env) : a;
    }
    case "or": {
      const a = evaluate(args[0], env);
      return isTruthy(a) ? a : evaluate(args[1], env);
    }
    case "not":
      return !isTruthy(evaluate(args[0], env));

    case "array":
      return args.map((v) => evaluate(v, env));

    case "record": {
      const result: { [key: string]: Value } = {};
      for (let i = 0; i + 1 < args.length; i += 2) {
        const key = asStr(evaluate(args[i], env));
        if (key !== undefined) result[key] = evaluate(args[i + 1], env);
      }
      return result;
    }

    case "concat": {
      let result = "";
      for (const arg of args) result += String(evaluate(arg, env) ?? "null");
      return result;
    }

    case "match": {
      const scrutinee = evaluate(args[0], env);
      for (let i = 1; i < args.length; i++) {
        const arm = asArray(args[i]);
        if (!arm || arm.length !== 2) continue;
        const mark = envPushScope(env);
        if (matchPattern(arm[0], scrutinee, env)) {
          const result = evaluate(arm[1], env);
          envPopScope(env, mark);
          return result;
        }
        envPopScope(env, mark);
      }
      return null;
    }

    case "perform": {
      const tag = asStr(args[0] as Value);
      switch (tag) {
        case "set": {
          if (args.length >= 3) {
            const key = asStr(evaluate(args[1], env)) ?? "";
            const value = evaluate(args[2], env);
            env.effects.push({ type: "set", key, value });
          }
          break;
        }
        case "send": {
          if (args.length >= 4) {
            const to = asStr(evaluate(args[1], env)) ?? "";
            const verb = asStr(evaluate(args[2], env)) ?? "";
            const payload = evaluate(args[3], env);
            env.effects.push({
              type: "send",
              to,
              message: { verb, payload } satisfies Message,
            });
          }
          break;
        }
        case "remove": {
          if (args.length >= 2) {
            const id = asStr(evaluate(args[1], env)) ?? "";
            env.effects.push({ type: "remove", id });
          }
          break;
        }
      }
      return null;
    }

    default:
      return null;
  }
}

function numericBinop(op: string, args: Value[], env: Env): Value {
  const a = evaluate(args[0], env);
  const b = evaluate(args[1], env);
  const an = asNum(a) ?? 0;
  const bn = asNum(b) ?? 0;
  switch (op) {
    case "+":
      return an + bn;
    case "-":
      return an - bn;
    case "*":
      return an * bn;
    case "/":
      return bn !== 0 ? an / bn : 0;
    default:
      return 0;
  }
}

function compareOp(
  args: Value[],
  env: Env,
  pred: (o: number) => boolean,
): boolean {
  const a = asNum(evaluate(args[0], env)) ?? 0;
  const b = asNum(evaluate(args[1], env)) ?? 0;
  return pred(a - b);
}

function deepEqual(a: Value, b: Value): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  if (typeof a === "object" && typeof b === "object") {
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    return ka.every((k) => deepEqual((a as any)[k], (b as any)[k]));
  }
  return false;
}

function matchPattern(pattern: Value, scrutinee: Value, env: Env): boolean {
  if (typeof pattern === "string") {
    if (pattern === "_") return true;
    // $ prefix = binding
    if (pattern.startsWith("$")) {
      envBind(env, pattern, scrutinee);
      return true;
    }
    return pattern === scrutinee;
  }
  if (
    pattern === null ||
    typeof pattern === "boolean" ||
    typeof pattern === "number"
  ) {
    return pattern === scrutinee;
  }
  if (Array.isArray(pattern)) {
    const sa = asArray(scrutinee);
    if (!sa || sa.length !== pattern.length) return false;
    return pattern.every((p, i) => matchPattern(p, sa[i], env));
  }
  if (typeof pattern === "object") {
    const sr = asRecord(scrutinee);
    if (!sr) return false;
    return Object.entries(pattern).every(
      ([k, p]) => k in sr && matchPattern(p, sr[k], env),
    );
  }
  return false;
}
