import type { Row } from "$lib/api";
import { CHUNK, chunksFor, type Range } from "./window";

type Fetch = (start: number, count: number) => Promise<Row[]>;

/** Graph rows fetched on demand in chunks; each chunk is requested once. */
export class RowStore {
  #chunks = new Map<number, Row[]>();
  #pending = new Set<number>();
  #fetch: Fetch;
  /** Bumped when a chunk lands, so views re-read. */
  version = $state(0);
  error = $state<string | null>(null);

  constructor(fetch: Fetch) {
    this.#fetch = fetch;
  }

  get(index: number): Row | undefined {
    return this.#chunks.get(Math.floor(index / CHUNK))?.[index % CHUNK];
  }

  /** Requests every chunk in `range` that isn't loaded or in flight. */
  ensure(range: Range): Promise<void> {
    const wanted = chunksFor(range).filter((c) => !this.#chunks.has(c) && !this.#pending.has(c));
    return Promise.all(wanted.map((c) => this.#load(c))).then(() => undefined);
  }

  /** Drops every loaded chunk, e.g. when a plan preview changes; loads in flight are ignored. */
  reset() {
    this.#generation++;
    this.#chunks.clear();
    this.#pending.clear();
    this.error = null;
    this.version++;
  }

  #generation = 0;

  async #load(chunk: number) {
    const generation = this.#generation;
    this.#pending.add(chunk);
    try {
      const rows = await this.#fetch(chunk * CHUNK, CHUNK);
      if (generation === this.#generation) {
        this.#chunks.set(chunk, rows);
        this.version++;
      }
    } catch (err) {
      if (generation === this.#generation) {
        this.error = String(err);
      }
    } finally {
      if (generation === this.#generation) {
        this.#pending.delete(chunk);
      }
    }
  }
}
