import { describe, test, expect } from "bun:test";
import { LocalStorageBackend } from "./persist-browser.js";
import { MemoryBackend, Tee } from "./persist.js";

/** Minimal Storage mock backed by a Map. */
class MockStorage implements Storage {
  private data = new Map<string, string>();

  get length(): number {
    return this.data.size;
  }

  clear(): void {
    this.data.clear();
  }

  getItem(key: string): string | null {
    return this.data.get(key) ?? null;
  }

  key(index: number): string | null {
    const keys = [...this.data.keys()];
    return keys[index] ?? null;
  }

  removeItem(key: string): void {
    this.data.delete(key);
  }

  setItem(key: string, value: string): void {
    this.data.set(key, value);
  }
}

describe("LocalStorageBackend", () => {
  test("save/load/remove round-trip", () => {
    const backend = new LocalStorageBackend({ storage: new MockStorage() });

    expect(backend.load("key")).toBeNull();
    backend.save("key", "value");
    expect(backend.load("key")).toBe("value");
    backend.remove("key");
    expect(backend.load("key")).toBeNull();
  });

  test("list returns saved keys", () => {
    const backend = new LocalStorageBackend({ storage: new MockStorage() });

    backend.save("a", "1");
    backend.save("b", "2");
    expect(backend.list().sort()).toEqual(["a", "b"]);
  });

  test("prefix namespacing isolates backends", () => {
    const storage = new MockStorage();
    const backendA = new LocalStorageBackend({ prefix: "game1-", storage });
    const backendB = new LocalStorageBackend({ prefix: "game2-", storage });

    backendA.save("slot", "world-a");
    backendB.save("slot", "world-b");

    expect(backendA.load("slot")).toBe("world-a");
    expect(backendB.load("slot")).toBe("world-b");
    expect(backendA.list()).toEqual(["slot"]);
    expect(backendB.list()).toEqual(["slot"]);
  });

  test("remove does not affect other prefixes", () => {
    const storage = new MockStorage();
    const backendA = new LocalStorageBackend({ prefix: "a-", storage });
    const backendB = new LocalStorageBackend({ prefix: "b-", storage });

    backendA.save("key", "val-a");
    backendB.save("key", "val-b");
    backendA.remove("key");

    expect(backendA.load("key")).toBeNull();
    expect(backendB.load("key")).toBe("val-b");
  });

  test("tee with localStorage and memory backend", () => {
    const ls = new LocalStorageBackend({ storage: new MockStorage() });
    const mem = new MemoryBackend();
    const tee = new Tee(ls, mem);

    tee.save("save-1", '{"version":1,"objects":{}}');
    expect(ls.load("save-1")).toBe('{"version":1,"objects":{}}');
    expect(mem.load("save-1")).toBe('{"version":1,"objects":{}}');

    tee.remove("save-1");
    expect(ls.load("save-1")).toBeNull();
    expect(mem.load("save-1")).toBeNull();
  });

  test("tee list combines keys from both backends", () => {
    const ls = new LocalStorageBackend({ storage: new MockStorage() });
    const mem = new MemoryBackend();
    const tee = new Tee(ls, mem);

    ls.save("only-ls", "1");
    mem.save("only-mem", "2");
    tee.save("both", "3");

    expect(tee.list().sort()).toEqual(["both", "only-ls", "only-mem"]);
  });
});
