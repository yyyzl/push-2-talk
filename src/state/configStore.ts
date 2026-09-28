/** Configuration snapshots are authoritative; only explicit edits become partial writes. */
export type DeepPatch<T> = T extends readonly unknown[] ? T : T extends object ? { [K in keyof T]?: DeepPatch<T[K]> | null } : T;
export type ConfigSnapshot<T> = { revision: number; config: T };
export type ConfigView<T> = ConfigSnapshot<T> & { loaded: boolean; dirty: boolean; saving: boolean; editVersion: number };
export type ConfigTransport<T> = (patch: DeepPatch<T>) => Promise<ConfigSnapshot<T>>;
type Edit = { path: string[]; value: unknown; version: number };
const object = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const equal = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b);
const clone = <T>(value: T): T => structuredClone(value);

function differences(before: unknown, after: unknown, path: string[] = []): { path: string[]; value: unknown }[] {
  if (equal(before, after)) return [];
  if (object(before) && object(after)) {
    return [...new Set([...Object.keys(before), ...Object.keys(after)])].flatMap(key => differences(before[key], after[key], [...path, key]));
  }
  // null explicitly clears optional values. Undefined would disappear during JSON serialization.
  return [{ path, value: after === undefined ? null : clone(after) }];
}
function read(root: unknown, path: string[]): unknown {
  return path.reduce<unknown>((value, key) => object(value) ? value[key] : undefined, root);
}
function write(root: object, path: string[], value: unknown): void {
  let cursor = root as Record<string, unknown>;
  for (const key of path.slice(0, -1)) {
    if (!Object.prototype.hasOwnProperty.call(cursor, key) || !object(cursor[key])) {
      Object.defineProperty(cursor, key, { value: {}, enumerable: true, writable: true, configurable: true });
    }
    cursor = cursor[key] as Record<string, unknown>;
  }
  Object.defineProperty(cursor, path[path.length - 1], { value: clone(value), enumerable: true, writable: true, configurable: true });
}

export class ConfigStore<T extends object> {
  private base: T;
  private view: ConfigView<T>;
  private edits = new Map<string, Edit>();
  private listeners = new Set<() => void>();
  private pending: Promise<T> | null = null;

  constructor(initial: T) {
    this.base = clone(initial);
    this.view = { config: clone(initial), revision: -1, loaded: false, dirty: false, saving: false, editVersion: 0 };
  }
  getSnapshot = (): ConfigView<T> => this.view;
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };

  private publish(changes: Partial<ConfigView<T>> = {}): void {
    const config = clone(this.base);
    for (const edit of this.edits.values()) write(config, edit.path, edit.value);
    this.view = { ...this.view, ...changes, config, dirty: this.edits.size > 0 };
    for (const listener of this.listeners) listener();
  }

  receive(snapshot: ConfigSnapshot<T>): void {
    if (snapshot.revision < this.view.revision) return;
    this.base = clone(snapshot.config);
    this.publish({ revision: snapshot.revision, loaded: true });
  }

  edit(change: (current: T) => T, observed: T = this.view.config): void {
    const next = change(clone(observed));
    // Compare with what the UI actually rendered; a concurrent credential refresh isn't a user edit.
    const changes = differences(observed, next).filter(change => !equal(read(this.view.config, change.path), change.value));
    if (changes.length === 0) return;
    const version = this.view.editVersion + 1;
    for (const change of changes) {
      const key = JSON.stringify(change.path);
      // Replacing a parent (e.g. clearing an optional feature override) supersedes its child edits.
      for (const [existing, edit] of this.edits) {
        if (change.path.every((part, index) => edit.path[index] === part)) this.edits.delete(existing);
      }
      if (!this.view.saving && equal(read(this.base, change.path), change.value)) this.edits.delete(key);
      else this.edits.set(key, { ...change, version });
    }
    this.publish({ editVersion: version });
  }

  flush(transport: ConfigTransport<T>): Promise<T> {
    if (this.pending) return this.pending;
    if (!this.view.loaded) return Promise.reject(new Error("配置尚未加载，不能保存"));
    if (!this.edits.size) return Promise.resolve(this.view.config);
    let firstBatch: Map<string, Edit> | null = new Map(this.edits);
    this.publish({ saving: true });
    const run = async (): Promise<T> => {
      // Install pending before calling transport, including transports which throw synchronously.
      await Promise.resolve();
      try {
        while (this.edits.size > 0) {
          const batch = firstBatch ?? new Map(this.edits);
          firstBatch = null;
          const patch = {};
          for (const edit of batch.values()) write(patch, edit.path, edit.value);
          const snapshot = await transport(patch as DeepPatch<T>);
          // An event may already have delivered this or a newer revision during the request.
          if (snapshot.revision >= this.view.revision) {
            this.base = clone(snapshot.config);
            this.view = { ...this.view, revision: snapshot.revision };
          }
          for (const [key, edit] of batch) {
            if (this.edits.get(key)?.version === edit.version) this.edits.delete(key);
          }
          this.publish();
        }
        return this.view.config;
      } finally {
        this.pending = null;
        this.publish({ saving: false });
      }
    };
    this.pending = run();
    return this.pending;
  }
}
