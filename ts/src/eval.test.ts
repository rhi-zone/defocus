import { describe, test, expect } from "bun:test";
import { evalHandler } from "./eval.js";
import type { Value, Expr } from "./value.js";

function run(expr: Expr): Value {
  // Use evalHandler but ignore effects; we just want the return value.
  // evalHandler doesn't return the value directly, so we wrap in a reply effect.
  const effects = evalHandler(
    ["do", expr, ["perform", "reply", expr]],
    null,
    null,
  );
  // Actually, evalHandler returns effects not the value.
  // We need a different approach — let's use the internal evaluate via a trick:
  // wrap the expression as ["perform", "reply", expr] and read the reply effect.
  const effects2 = evalHandler(["perform", "reply", expr], null, null);
  if (effects2.length > 0 && effects2[0].type === "reply") {
    return effects2[0].value;
  }
  return null;
}

// A simpler approach: use let + perform reply to capture the result
function evalExpr(expr: Expr): Value {
  const effects = evalHandler(["perform", "reply", expr], null, null);
  if (effects.length > 0 && effects[0].type === "reply") {
    return effects[0].value;
  }
  return null;
}

describe("Closures", () => {
  test("fn captures environment", () => {
    const result = evalExpr([
      "let",
      "x",
      10,
      [
        "let",
        "add-x",
        ["fn", ["y"], ["+", ["get", "x"], ["get", "y"]]],
        ["call", ["get", "add-x"], 5],
      ],
    ]);
    expect(result).toBe(15);
  });

  test("closure doesn't leak later bindings", () => {
    const result = evalExpr([
      "let",
      "make-fn",
      ["fn", [], ["get", "z"]],
      ["let", "z", 999, ["call", ["get", "make-fn"]]],
    ]);
    expect(result).toBeNull();
  });
});

describe("Map/Filter/Reduce", () => {
  test("map doubles elements", () => {
    const result = evalExpr([
      "map",
      [1, 2, 3],
      ["fn", ["x"], ["*", ["get", "x"], 2]],
    ]);
    expect(result).toEqual([2, 4, 6]);
  });

  test("filter keeps elements > 2", () => {
    const result = evalExpr([
      "filter",
      [1, 2, 3, 4, 5],
      ["fn", ["x"], [">", ["get", "x"], 2]],
    ]);
    expect(result).toEqual([3, 4, 5]);
  });

  test("reduce sums elements", () => {
    const result = evalExpr([
      "reduce",
      [1, 2, 3, 4, 5],
      ["fn", ["acc", "x"], ["+", ["get", "acc"], ["get", "x"]]],
      0,
    ]);
    expect(result).toBe(15);
  });

  test("length of array", () => {
    const result = evalExpr(["length", [1, 2, 3]]);
    expect(result).toBe(3);
  });

  test("length of string", () => {
    const result = evalExpr(["length", "hello"]);
    expect(result).toBe(5);
  });
});

describe("Record operations", () => {
  test("keys returns key array", () => {
    const result = evalExpr(["keys", ["record", "a", 1, "b", 2]]);
    expect(result).toEqual(["a", "b"]);
  });

  test("values returns value array", () => {
    const result = evalExpr(["values", ["record", "a", 1, "b", 2]]);
    expect(result).toEqual([1, 2]);
  });

  test("has returns true for existing key", () => {
    const result = evalExpr(["has", ["record", "a", 1, "b", 2], "a"]);
    expect(result).toBe(true);
  });

  test("has returns false for missing key", () => {
    const result = evalExpr(["has", ["record", "a", 1, "b", 2], "c"]);
    expect(result).toBe(false);
  });

  test("set-in adds a new key", () => {
    const result = evalExpr(["set-in", ["record", "a", 1], "b", 2]);
    expect(result).toEqual({ a: 1, b: 2 });
  });

  test("set-in updates an existing key", () => {
    const result = evalExpr(["set-in", ["record", "a", 1], "a", 99]);
    expect(result).toEqual({ a: 99 });
  });

  test("remove-key removes a key", () => {
    const result = evalExpr(["remove-key", ["record", "a", 1, "b", 2], "a"]);
    expect(result).toEqual({ b: 2 });
  });
});
