import { loadMeshBundle } from "./fbxPool";
import { type LoadedModel, ModelLoadError } from "./load";
import { unpackFbx } from "./packFbx";

/** Meshes per request: few requests, yet a scene still fills in nearest first. */
const PER_BUNDLE = 32;

/**
 * Loads a scene's own meshes (`fileID`s inside the file at `path`) in bundles: those
 * asked for in the same task go out together, in the order asked, a few dozen per request.
 */
export function inlineBatcher(urlFor: (path: string, ids: string[]) => string): (path: string, id: string) => Promise<LoadedModel> {
  let queue: { path: string; id: string; resolve: (m: LoadedModel) => void; reject: (e: Error) => void }[] = [];

  const send = (path: string, batch: typeof queue) => {
    loadMeshBundle(
      urlFor(
        path,
        batch.map((b) => b.id),
      ),
    ).then(
      (meshes) => {
        const byId = new Map(meshes.map((m) => [m.id, m]));
        for (const b of batch) {
          const m = byId.get(b.id);
          if (m && "packed" in m) {
            b.resolve(unpackFbx(m.packed, 1));
          } else {
            b.reject(new ModelLoadError(m?.error ?? `${path} sent nothing for Mesh &${b.id}`));
          }
        }
      },
      (e) => {
        const err = e instanceof Error ? e : new Error(String(e));
        batch.forEach((b) => b.reject(err));
      },
    );
  };

  const flush = () => {
    const all = queue;
    queue = [];
    const byPath = new Map<string, typeof queue>();
    for (const q of all) {
      byPath.set(q.path, [...(byPath.get(q.path) ?? []), q]);
    }
    for (const [path, list] of byPath) {
      for (let i = 0; i < list.length; i += PER_BUNDLE) {
        send(path, list.slice(i, i + PER_BUNDLE));
      }
    }
  };

  return (path, id) =>
    new Promise((resolve, reject) => {
      if (queue.length === 0) {
        setTimeout(flush, 0);
      }
      queue.push({ path, id, resolve, reject });
    });
}
