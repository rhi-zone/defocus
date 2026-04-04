import { World } from "./world.js";

/** Minimal persistence backend. Borrowed from reincarnate's SaveBackend pattern. */
export interface SaveBackend {
  load(key: string): string | null;
  save(key: string, value: string): void;
  remove(key: string): void;
  list(): string[];
}

/** In-memory backend for testing and embedded use. */
export class MemoryBackend implements SaveBackend {
  private store = new Map<string, string>();

  load(key: string): string | null {
    return this.store.get(key) ?? null;
  }
  save(key: string, value: string): void {
    this.store.set(key, value);
  }
  remove(key: string): void {
    this.store.delete(key);
  }
  list(): string[] {
    return [...this.store.keys()];
  }
}

/** Fan-out writes to multiple backends. */
export class Tee implements SaveBackend {
  constructor(
    public a: SaveBackend,
    public b: SaveBackend,
  ) {}

  load(key: string): string | null {
    return this.a.load(key) ?? this.b.load(key);
  }
  save(key: string, value: string): void {
    this.a.save(key, value);
    this.b.save(key, value);
  }
  remove(key: string): void {
    this.a.remove(key);
    this.b.remove(key);
  }
  list(): string[] {
    const keys = new Set(this.a.list());
    for (const k of this.b.list()) keys.add(k);
    return [...keys];
  }
}

/** Keep last N saves per prefix, pruning oldest. */
export class Rolling implements SaveBackend {
  constructor(
    public inner: SaveBackend,
    public max: number,
    public prefix: string,
  ) {}

  load(key: string): string | null {
    return this.inner.load(key);
  }

  save(key: string, value: string): void {
    this.inner.save(key, value);
    const slots = this.inner
      .list()
      .filter((k) => k.startsWith(this.prefix))
      .map((k) => {
        const n = parseInt(k.slice(this.prefix.length), 10);
        return isNaN(n) ? null : { n, k };
      })
      .filter((x): x is { n: number; k: string } => x !== null)
      .sort((a, b) => a.n - b.n);

    while (slots.length > this.max) {
      this.inner.remove(slots[0].k);
      slots.shift();
    }
  }

  remove(key: string): void {
    this.inner.remove(key);
  }

  list(): string[] {
    return this.inner.list();
  }
}

/** Save a world to a backend. */
export function saveWorld(
  world: World,
  backend: SaveBackend,
  key: string,
): void {
  backend.save(key, JSON.stringify(world.toJSON()));
}

/** Load a world from a backend. */
export function loadWorld(
  backend: SaveBackend,
  key: string,
): World | null {
  const data = backend.load(key);
  if (data === null) return null;
  return World.fromJSON(JSON.parse(data));
}
