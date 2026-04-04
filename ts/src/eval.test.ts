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

describe("Try (null coalescing)", () => {
  test("null falls back to fallback", () => {
    expect(evalExpr(["try", null, 42])).toBe(42);
  });

  test("non-null returns value", () => {
    expect(evalExpr(["try", "hello", 42])).toBe("hello");
  });

  test("false is not null", () => {
    expect(evalExpr(["try", false, 42])).toBe(false);
  });
});

describe("Type checking", () => {
  test("type returns correct type strings", () => {
    expect(evalExpr(["type", null])).toBe("null");
    expect(evalExpr(["type", true])).toBe("bool");
    expect(evalExpr(["type", 42])).toBe("int");
    expect(evalExpr(["type", 3.14])).toBe("float");
    expect(evalExpr(["type", "hello"])).toBe("string");
    expect(evalExpr(["type", [1, 2, 3]])).toBe("array");
    expect(evalExpr(["type", ["record", "a", 1]])).toBe("record");
  });

  test("is checks type correctly", () => {
    expect(evalExpr(["is", "string", "hello"])).toBe(true);
    expect(evalExpr(["is", "int", "hello"])).toBe(false);
    expect(evalExpr(["is", "null", null])).toBe(true);
  });
});

describe("String operations", () => {
  test("split and join round-trip", () => {
    expect(evalExpr(["split", "a,b,c", ","])).toEqual(["a", "b", "c"]);
    expect(evalExpr(["join", ["split", "a,b,c", ","], "-"])).toBe("a-b-c");
  });

  test("trim", () => {
    expect(evalExpr(["trim", "  hello  "])).toBe("hello");
  });

  test("starts-with / ends-with", () => {
    expect(evalExpr(["starts-with", "hello world", "hello"])).toBe(true);
    expect(evalExpr(["starts-with", "hello world", "world"])).toBe(false);
    expect(evalExpr(["ends-with", "hello world", "world"])).toBe(true);
  });

  test("slice string", () => {
    expect(evalExpr(["slice", "hello", 1, 3])).toBe("el");
    expect(evalExpr(["slice", "hello", 2])).toBe("llo");
  });

  test("upper / lower", () => {
    expect(evalExpr(["upper", "hello"])).toBe("HELLO");
    expect(evalExpr(["lower", "HELLO"])).toBe("hello");
  });
});

describe("Number operations", () => {
  test("floor/ceil/round", () => {
    expect(evalExpr(["floor", 3.7])).toBe(3);
    expect(evalExpr(["ceil", 3.2])).toBe(4);
    expect(evalExpr(["round", 3.5])).toBe(4);
    expect(evalExpr(["round", 3.4])).toBe(3);
  });

  test("abs", () => {
    expect(evalExpr(["abs", -5])).toBe(5);
    expect(evalExpr(["abs", 5])).toBe(5);
  });

  test("min/max", () => {
    expect(evalExpr(["min", 3, 7])).toBe(3);
    expect(evalExpr(["max", 3, 7])).toBe(7);
  });

  test("mod", () => {
    expect(evalExpr(["mod", 10, 3])).toBe(1);
  });
});

describe("Array operations (new)", () => {
  test("push appends element", () => {
    expect(evalExpr(["push", [1, 2], 3])).toEqual([1, 2, 3]);
  });

  test("nth gets element by index", () => {
    expect(evalExpr(["nth", [10, 20, 30], 1])).toBe(20);
    expect(evalExpr(["nth", [10, 20, 30], 5])).toBeNull();
  });

  test("range generates array", () => {
    expect(evalExpr(["range", 0, 4])).toEqual([0, 1, 2, 3]);
  });

  test("flat flattens one level", () => {
    expect(evalExpr(["flat", [[1, 2], [3, 4], 5]])).toEqual([1, 2, 3, 4, 5]);
  });

  test("sort numbers and strings", () => {
    expect(evalExpr(["sort", [3, 1, 2]])).toEqual([1, 2, 3]);
    expect(evalExpr(["sort", ["array", "c", "a", "b"]])).toEqual(["a", "b", "c"]);
  });

  test("reverse", () => {
    expect(evalExpr(["reverse", [1, 2, 3]])).toEqual([3, 2, 1]);
  });

  test("slice array", () => {
    expect(evalExpr(["slice", [10, 20, 30, 40], 1, 3])).toEqual([20, 30]);
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
