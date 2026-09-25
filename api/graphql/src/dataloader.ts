/**
 * Minimal request-scoped DataLoader implementation.
 *
 * It intentionally has no package dependency: keys are cached and duplicate
 * keys share one promise, while queued keys are dispatched in one batch on the
 * next microtask.
 */
export type BatchLoadFn<K, V> = (keys: readonly K[]) => Promise<ReadonlyArray<V | Error>>;

export class DataLoader<K, V> {
  private readonly cache = new Map<K, {
    promise: Promise<V>;
    resolve: (value: V) => void;
    reject: (reason?: unknown) => void;
  }>();
  private queue: K[] = [];
  private dispatched = false;

  public constructor(private readonly batchLoadFn: BatchLoadFn<K, V>) {}

  public load(key: K): Promise<V> {
    const cached = this.cache.get(key);
    if (cached) return cached.promise;

    let resolve!: (value: V) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<V>((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    this.cache.set(key, { promise, resolve, reject });
    this.queue.push(key);
    if (!this.dispatched) {
      this.dispatched = true;
      queueMicrotask(() => this.dispatch());
    }
    return promise;
  }

  public async loadMany(keys: readonly K[]): Promise<Array<V | Error>> {
    return Promise.all(keys.map((key) => this.load(key).catch((error) => error)));
  }

  public clear(key: K): void {
    this.cache.delete(key);
  }

  public clearAll(): void {
    this.cache.clear();
  }

  private async dispatch(): Promise<void> {
    const keys = this.queue;
    this.queue = [];
    this.dispatched = false;
    try {
      const values = await this.batchLoadFn(keys);
      if (values.length !== keys.length) {
        throw new Error(`DataLoader batch returned ${values.length} values for ${keys.length} keys`);
      }
      keys.forEach((key, index) => {
        const entry = this.cache.get(key);
        if (!entry) return;
        const value = values[index];
        if (value instanceof Error) {
          this.cache.delete(key);
          entry.reject(value);
        } else {
          entry.resolve(value as V);
        }
      });
    } catch (error) {
      for (const key of keys) {
        const entry = this.cache.get(key);
        if (entry) {
          this.cache.delete(key);
          entry.reject(error);
        }
      }
    }
  }
}
