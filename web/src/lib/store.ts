/**
 * The one observable primitive the stateful islands share.
 *
 * Deliberately not a signals library: the Preact islands need exactly "hold a value, update it,
 * re-render subscribers", and `useSyncExternalStore` is the platform-blessed way to couple that
 * to Preact without adding a reactivity runtime.
 */
import { useSyncExternalStore } from "preact/compat";

export class Store<T> {
  private value: T;
  private listeners = new Set<() => void>();

  constructor(initial: T) {
    this.value = initial;
  }

  get(): T {
    return this.value;
  }

  set(next: T): void {
    if (Object.is(next, this.value)) return;
    this.value = next;
    for (const listener of this.listeners) listener();
  }

  update(mutate: (current: T) => T): void {
    this.set(mutate(this.value));
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

/** Subscribe a Preact component to a store — re-renders on every `set`. */
export function useStore<T>(store: Store<T>): T {
  return useSyncExternalStore(
    (onChange) => store.subscribe(onChange),
    () => store.get(),
  );
}
