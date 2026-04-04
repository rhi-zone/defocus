import type { Value, Expr } from "./value.js";
import { isTruthy, asStr, asNum, asArray, asRecord, asRef, isRef, refVerbs } from "./value.js";
import type { DefocusObject, Effect, Message } from "./world.js";
import type { LlmProvider } from "./llm.js";

interface Env {
  bindings: Array<[string, Value]>;
  effects: Effect[];
  llm: LlmProvider | null;
}

function envNew(llm: LlmProvider | null = null): Env {
  return { bindings: [], effects: [], llm };
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
  selfId: string = "",
  sender: string | undefined = undefined,
): Effect[] {
  return evalHandlerWithLlm(handler, payload, state, selfId, sender, null);
}

export function evalHandlerWithLlm(
  handler: Expr,
  payload: Value,
  state: Value,
  selfId: string = "",
  sender: string | undefined = undefined,
  llm: LlmProvider | null = null,
): Effect[] {
  const env = envNew(llm);
  envBind(env, "self", { $ref: selfId });
  envBind(env, "sender", sender !== undefined ? { $ref: sender } : null);
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

    case "fn": {
      // ["fn", [params...], body] → ["$fn", [params...], body, captured-bindings]
      if (args.length < 2) return null;
      // Capture current bindings as a record
      const captured: { [key: string]: Value } = {};
      for (const [k, v] of env.bindings) {
        captured[k] = v;
      }
      return ["$fn", args[0], args[1], captured];
    }

    case "call": {
      // ["call", fn-expr, arg1, arg2, ...]
      if (args.length === 0) return null;
      const func = evaluate(args[0], env);
      const fnArr = asArray(func);
      if (!fnArr || fnArr.length < 3 || fnArr[0] !== "$fn") return null;
      const params = asArray(fnArr[1]);
      if (!params) return null;
      const body = fnArr[2];

      // Evaluate arguments in the current environment
      const evaluatedArgs = args.slice(1).map((a) => evaluate(a, env));

      // For closures (4-element $fn), replace entire binding environment
      // with captured bindings for proper lexical scoping.
      // For non-closures (3-element $fn, backward compat), just push a scope.
      if (fnArr.length >= 4) {
        const saved = env.bindings;
        env.bindings = [];
        const captured = asRecord(fnArr[3]);
        if (captured) {
          for (const [k, v] of Object.entries(captured)) {
            envBind(env, k, v);
          }
        }
        for (let i = 0; i < params.length; i++) {
          const name = asStr(params[i] as Value);
          if (name !== undefined) {
            envBind(env, name, evaluatedArgs[i] ?? null);
          }
        }
        const result = evaluate(body, env);
        env.bindings = saved;
        return result;
      } else {
        const mark = envPushScope(env);
        for (let i = 0; i < params.length; i++) {
          const name = asStr(params[i] as Value);
          if (name !== undefined) {
            envBind(env, name, evaluatedArgs[i] ?? null);
          }
        }
        const result = evaluate(body, env);
        envPopScope(env, mark);
        return result;
      }
    }

    // Array operations
    case "map": {
      const arrVal = evaluate(args[0], env);
      const func = evaluate(args[1], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      return arr.map((elem) => callFn(func, [elem], env));
    }

    case "filter": {
      const arrVal = evaluate(args[0], env);
      const func = evaluate(args[1], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      return arr.filter((elem) => isTruthy(callFn(func, [elem], env)));
    }

    case "reduce": {
      const arrVal = evaluate(args[0], env);
      const func = evaluate(args[1], env);
      const init = evaluate(args[2], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      let acc = init;
      for (const elem of arr) {
        acc = callFn(func, [acc, elem], env);
      }
      return acc;
    }

    case "length": {
      const val = evaluate(args[0], env);
      if (Array.isArray(val)) return val.length;
      if (typeof val === "string") return val.length;
      return null;
    }

    // Record operations
    case "keys": {
      const val = evaluate(args[0], env);
      const rec = asRecord(val);
      if (!rec) return null;
      return Object.keys(rec);
    }

    case "values": {
      const val = evaluate(args[0], env);
      const rec = asRecord(val);
      if (!rec) return null;
      return Object.values(rec);
    }

    case "has": {
      const val = evaluate(args[0], env);
      const key = evaluate(args[1], env);
      const rec = asRecord(val);
      const k = asStr(key);
      if (!rec || k === undefined) return false;
      return k in rec;
    }

    case "set-in": {
      const val = evaluate(args[0], env);
      const key = evaluate(args[1], env);
      const value = evaluate(args[2], env);
      const rec = asRecord(val);
      const k = asStr(key);
      if (!rec || k === undefined) return null;
      return { ...rec, [k]: value };
    }

    case "remove-key": {
      const val = evaluate(args[0], env);
      const key = evaluate(args[1], env);
      const rec = asRecord(val);
      const k = asStr(key);
      if (!rec || k === undefined) return null;
      const { [k]: _, ...rest } = rec;
      return rest;
    }

    case "attenuate": {
      // ["attenuate", ref-expr, ["verb1", "verb2"]]
      if (args.length < 2) return null;
      const refVal = evaluate(args[0], env);
      const verbsVal = evaluate(args[1], env);
      const newVerbsArr = asArray(verbsVal);
      if (!newVerbsArr || !isRef(refVal)) return null;
      const newVerbs = newVerbsArr.filter((v): v is string => typeof v === "string");
      const existing = refVerbs(refVal);
      const id = asRef(refVal)!;
      const finalVerbs = existing
        ? newVerbs.filter((v) => existing.includes(v))
        : newVerbs;
      const result: Value = { $ref: id };
      if (finalVerbs.length > 0 || existing || newVerbs.length > 0) {
        (result as any).$verbs = finalVerbs;
      }
      return result;
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
            const target = evaluate(args[1], env);
            const to = asRef(target) ?? asStr(target) ?? "";
            const allowedVerbs = isRef(target) ? refVerbs(target) : undefined;
            const verb = asStr(evaluate(args[2], env)) ?? "";
            const payload = evaluate(args[3], env);
            env.effects.push({
              type: "send",
              to,
              allowedVerbs,
              message: { verb, payload } satisfies Message,
            });
          }
          break;
        }
        case "reply": {
          if (args.length >= 2) {
            const value = evaluate(args[1], env);
            env.effects.push({ type: "reply", value });
          }
          break;
        }
        case "schedule": {
          // ["perform", "schedule", tick-expr, ref-or-id, verb, payload]
          if (args.length >= 5) {
            const at = asNum(evaluate(args[1], env)) ?? 0;
            const target = evaluate(args[2], env);
            const to = asRef(target) ?? asStr(target) ?? "";
            const verb = asStr(evaluate(args[3], env)) ?? "";
            const payload = evaluate(args[4], env);
            env.effects.push({
              type: "schedule",
              at,
              to,
              message: { verb, payload } satisfies Message,
            });
          }
          break;
        }
        case "remove": {
          if (args.length >= 2) {
            const target = evaluate(args[1], env);
            const id = asRef(target) ?? asStr(target) ?? "";
            env.effects.push({ type: "remove", id });
          }
          break;
        }
        case "spawn": {
          if (args.length >= 3) {
            const target = evaluate(args[1], env);
            const id = asRef(target) ?? asStr(target) ?? "";
            // Don't fully evaluate the spec — handlers are stored
            // as unevaluated expressions, interface is data.
            // Only evaluate state values (for computed initial state).
            const spec = asRecord(args[2]);
            if (spec) {
              const stateRec = asRecord(spec.state);
              const state: { [key: string]: Value } = {};
              if (stateRec) {
                for (const [k, v] of Object.entries(stateRec)) {
                  state[k] = evaluate(v, env);
                }
              }
              const handlersRec = asRecord(spec.handlers);
              const handlers: { [verb: string]: Value } = {};
              if (handlersRec) {
                for (const [k, v] of Object.entries(handlersRec)) {
                  handlers[k] = v; // Not evaluated — stored for later
                }
              }
              const ifaceArr = asArray(spec.interface);
              const iface: string[] = [];
              if (ifaceArr) {
                for (const v of ifaceArr) {
                  const s = asStr(v);
                  if (s !== undefined) iface.push(s);
                }
              }
              const protoVal = spec.prototype;
              const proto = asRef(protoVal) ?? asStr(protoVal) ?? null;
              const object: DefocusObject = {
                id,
                state,
                handlers,
                interface: iface,
                children: [],
                prototype: proto,
              };
              env.effects.push({ type: "spawn", object });
              return { $ref: id };
            }
          }
          break;
        }
      }
      return null;
    }

    // Null coalescing: ["try", expr, fallback-expr]
    case "try": {
      const result = evaluate(args[0], env);
      return result === null ? evaluate(args[1], env) : result;
    }

    // Type checking
    case "type": {
      const val = evaluate(args[0], env);
      if (val === null) return "null";
      if (typeof val === "boolean") return "bool";
      if (typeof val === "number") return Number.isInteger(val) ? "int" : "float";
      if (typeof val === "string") return "string";
      if (Array.isArray(val)) return "array";
      if (isRef(val)) return "ref";
      return "record";
    }

    case "is": {
      const typeName = evaluate(args[0], env);
      const val = evaluate(args[1], env);
      if (typeof typeName !== "string") return false;
      switch (typeName) {
        case "null": return val === null;
        case "bool": return typeof val === "boolean";
        case "int": return typeof val === "number" && Number.isInteger(val);
        case "float": return typeof val === "number" && !Number.isInteger(val);
        case "string": return typeof val === "string";
        case "array": return Array.isArray(val);
        case "record": return val !== null && typeof val === "object" && !Array.isArray(val) && !isRef(val);
        case "ref": return isRef(val);
        default: return false;
      }
    }

    // String operations
    case "split": {
      const val = evaluate(args[0], env);
      const sep = evaluate(args[1], env);
      if (typeof val !== "string" || typeof sep !== "string") return null;
      return val.split(sep);
    }

    case "join": {
      const val = evaluate(args[0], env);
      const sep = evaluate(args[1], env);
      const arr = asArray(val);
      if (!arr || typeof sep !== "string") return null;
      return arr.map((v) => String(v ?? "null")).join(sep);
    }

    case "trim": {
      const val = evaluate(args[0], env);
      if (typeof val !== "string") return null;
      return val.trim();
    }

    case "starts-with": {
      const val = evaluate(args[0], env);
      const prefix = evaluate(args[1], env);
      if (typeof val !== "string" || typeof prefix !== "string") return false;
      return val.startsWith(prefix);
    }

    case "ends-with": {
      const val = evaluate(args[0], env);
      const suffix = evaluate(args[1], env);
      if (typeof val !== "string" || typeof suffix !== "string") return false;
      return val.endsWith(suffix);
    }

    case "slice": {
      const val = evaluate(args[0], env);
      const start = asNum(evaluate(args[1], env)) ?? 0;
      const endVal = args.length > 2 ? asNum(evaluate(args[2], env)) : undefined;
      if (typeof val === "string") {
        return val.slice(start, endVal);
      }
      const arr = asArray(val);
      if (arr) {
        return arr.slice(start, endVal);
      }
      return null;
    }

    case "upper": {
      const val = evaluate(args[0], env);
      if (typeof val !== "string") return null;
      return val.toUpperCase();
    }

    case "lower": {
      const val = evaluate(args[0], env);
      if (typeof val !== "string") return null;
      return val.toLowerCase();
    }

    // Number operations
    case "floor": {
      const val = evaluate(args[0], env);
      const n = asNum(val);
      if (n === undefined) return null;
      return Math.floor(n);
    }

    case "ceil": {
      const val = evaluate(args[0], env);
      const n = asNum(val);
      if (n === undefined) return null;
      return Math.ceil(n);
    }

    case "round": {
      const val = evaluate(args[0], env);
      const n = asNum(val);
      if (n === undefined) return null;
      return Math.round(n);
    }

    case "abs": {
      const val = evaluate(args[0], env);
      const n = asNum(val);
      if (n === undefined) return null;
      return Math.abs(n);
    }

    case "min": {
      const a = asNum(evaluate(args[0], env)) ?? 0;
      const b = asNum(evaluate(args[1], env)) ?? 0;
      return Math.min(a, b);
    }

    case "max": {
      const a = asNum(evaluate(args[0], env)) ?? 0;
      const b = asNum(evaluate(args[1], env)) ?? 0;
      return Math.max(a, b);
    }

    case "mod": {
      const a = asNum(evaluate(args[0], env)) ?? 0;
      const b = asNum(evaluate(args[1], env)) ?? 0;
      return b !== 0 ? a % b : 0;
    }

    // Additional array operations
    case "push": {
      const arrVal = evaluate(args[0], env);
      const value = evaluate(args[1], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      return [...arr, value];
    }

    case "nth": {
      const arrVal = evaluate(args[0], env);
      const idx = evaluate(args[1], env);
      const arr = asArray(arrVal);
      if (!arr || typeof idx !== "number") return null;
      return arr[idx] ?? null;
    }

    case "range": {
      const start = asNum(evaluate(args[0], env)) ?? 0;
      const end = asNum(evaluate(args[1], env)) ?? 0;
      const result: Value[] = [];
      for (let i = start; i < end; i++) result.push(i);
      return result;
    }

    case "flat": {
      const arrVal = evaluate(args[0], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      const result: Value[] = [];
      for (const elem of arr) {
        const inner = asArray(elem);
        if (inner) {
          result.push(...inner);
        } else {
          result.push(elem);
        }
      }
      return result;
    }

    case "sort": {
      const arrVal = evaluate(args[0], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      return [...arr].sort((a, b) => {
        if (typeof a === "number" && typeof b === "number") return a - b;
        if (typeof a === "string" && typeof b === "string") return a < b ? -1 : a > b ? 1 : 0;
        return 0;
      });
    }

    case "reverse": {
      const arrVal = evaluate(args[0], env);
      const arr = asArray(arrVal);
      if (!arr) return null;
      return [...arr].reverse();
    }

    case "llm": {
      if (!env.llm) return null;
      const prompt = evaluate(args[0], env);
      const promptStr = String(prompt ?? "null");
      const result = env.llm.complete(promptStr);
      // Only support synchronous providers in the sync eval path.
      // If complete() returns a Promise, return null.
      if (typeof result === "string") return result;
      return null;
    }

    default:
      return null;
  }
}

function callFn(func: Value, callArgs: Value[], env: Env): Value {
  const fnArr = asArray(func);
  if (!fnArr || fnArr.length < 3 || fnArr[0] !== "$fn") return null;
  const params = asArray(fnArr[1]);
  if (!params) return null;
  const body = fnArr[2];

  if (fnArr.length >= 4) {
    const saved = env.bindings;
    env.bindings = [];
    const captured = asRecord(fnArr[3]);
    if (captured) {
      for (const [k, v] of Object.entries(captured)) {
        envBind(env, k, v);
      }
    }
    for (let i = 0; i < params.length; i++) {
      const name = asStr(params[i] as Value);
      if (name !== undefined) {
        envBind(env, name, callArgs[i] ?? null);
      }
    }
    const result = evaluate(body, env);
    env.bindings = saved;
    return result;
  } else {
    const mark = envPushScope(env);
    for (let i = 0; i < params.length; i++) {
      const name = asStr(params[i] as Value);
      if (name !== undefined) {
        envBind(env, name, callArgs[i] ?? null);
      }
    }
    const result = evaluate(body, env);
    envPopScope(env, mark);
    return result;
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
