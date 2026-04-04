import type { SaveBackend } from "./persist.js";

/** localStorage-backed persistence. Works in browsers and any environment with a Storage-compatible object. */
export class LocalStorageBackend implements SaveBackend {
  private storage: Storage;
  private prefix: string;

  constructor(options?: { prefix?: string; storage?: Storage }) {
    this.prefix = options?.prefix ?? "defocus-";
    this.storage = options?.storage ?? globalThis.localStorage;
  }

  load(key: string): string | null {
    return this.storage.getItem(this.prefix + key);
  }

  save(key: string, value: string): void {
    this.storage.setItem(this.prefix + key, value);
  }

  remove(key: string): void {
    this.storage.removeItem(this.prefix + key);
  }

  list(): string[] {
    const keys: string[] = [];
    for (let i = 0; i < this.storage.length; i++) {
      const k = this.storage.key(i);
      if (k !== null && k.startsWith(this.prefix)) {
        keys.push(k.slice(this.prefix.length));
      }
    }
    return keys;
  }
}

/** IndexedDB-backed persistence with synchronous reads from an in-memory cache. */
export class IndexedDbBackend implements SaveBackend {
  private cache = new Map<string, string>();
  private db: IDBDatabase | null = null;
  private dbName: string;
  private storeName = "saves";

  constructor(dbName: string) {
    this.dbName = dbName;
  }

  /** Open the database and preload all entries into the cache. Must be called before use. */
  async init(): Promise<void> {
    this.db = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open(this.dbName, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore(this.storeName);
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });

    // Preload all entries into cache
    const entries = await new Promise<Map<string, string>>((resolve, reject) => {
      const tx = this.db!.transaction(this.storeName, "readonly");
      const store = tx.objectStore(this.storeName);
      const request = store.openCursor();
      const result = new Map<string, string>();
      request.onsuccess = () => {
        const cursor = request.result;
        if (cursor) {
          result.set(cursor.key as string, cursor.value as string);
          cursor.continue();
        } else {
          resolve(result);
        }
      };
      request.onerror = () => reject(request.error);
    });

    this.cache = entries;
  }

  load(key: string): string | null {
    return this.cache.get(key) ?? null;
  }

  save(key: string, value: string): void {
    this.cache.set(key, value);
    if (this.db) {
      const tx = this.db.transaction(this.storeName, "readwrite");
      tx.objectStore(this.storeName).put(value, key);
    }
  }

  remove(key: string): void {
    this.cache.delete(key);
    if (this.db) {
      const tx = this.db.transaction(this.storeName, "readwrite");
      tx.objectStore(this.storeName).delete(key);
    }
  }

  list(): string[] {
    return [...this.cache.keys()];
  }
}
