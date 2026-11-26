import {
  ACESFilmicToneMapping,
  AnimationMixer,
  Box3,
  DirectionalLight,
  GridHelper,
  Group,
  HemisphereLight,
  type Material,
  Mesh,
  type Object3D,
  PCFSoftShadowMap,
  PerspectiveCamera,
  PlaneGeometry,
  PMREMGenerator,
  Raycaster,
  Scene,
  ShadowMaterial,
  Sphere,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { type Comparison, type MergeSource, bySource, compare, type Status } from "./compare";
import type { LoadedModel } from "./load";
import { type Instanced, instance } from "./instancing";
import { ShadingKit, textureEvents } from "./materials";
import { FOV, fitDistance, framingCamera } from "./view";

/** "merged": the newer version only, with the older one's changed objects as amber wire ghosts. */
export type CompareMode = "side" | "swipe" | "onion" | "merged";
export type Shading = "textured" | "changes";

/** How often pieces arriving into a loading model are drawn: at most every 500 ms, at least every second. */
const TRICKLE_MS = 500;
const TRICKLE_MAX_MS = 1000;

interface Side {
  scene: Scene;
  /** Lights, floor and grid, sized to the models. */
  rig: Group;
  helpers: Group;
  /** The unchanged static meshes drawn instanced, once the model is complete. */
  instanced: Instanced | null;
  model: LoadedModel;
  mixer: AnimationMixer;
}

interface Fly {
  from: Vector3;
  to: Vector3;
  camFrom: Vector3;
  camTo: Vector3;
  k: number;
}


/**
 * Draws one or two versions of a model into one canvas: side by side, swiped
 * or onion-skinned, with a shared camera and a shared animation clock.
 */
export class ModelStage {
  readonly renderer: WebGLRenderer;
  readonly camera = new PerspectiveCamera(FOV, 1, 0.01, 1000);
  readonly controls: OrbitControls;
  comparison: Comparison = { status: new Map(), changes: [] };
  /** When set, the Changes shading tints by merge source instead of comparing the sides. */
  private sources: Map<string, MergeSource> | null = null;
  mode: CompareMode = "side";
  shading: Shading = "textured";
  wire = false;
  split = 0.5;
  playing = false;
  time = 0;
  clip: string | null = null;
  /** Called every frame the clock moves, with the time and clip length. */
  ontime: (time: number, duration: number) => void = () => {};
  /** The user orbited, zoomed or flew to an object since the camera last framed everything. */
  cameraMoved = false;

  private sides: [Side | null, Side | null] = [null, null];
  private readonly kit = new ShadingKit();
  private readonly env;
  private width = 0;
  private height = 0;
  private fly: Fly | null = null;
  private dirty = true;
  private raf = 0;
  private last = 0;
  private active = true;
  private compiling: Promise<void> | null = null;
  /** The models finished loading (`refresh`), so their static meshes can be instanced. */
  private complete = false;
  /** Pieces of a loading model arrived since the last frame. */
  private trickled = false;
  /** The models changed or moved since the shadow maps were drawn. */
  private shadowsStale = true;
  /** When the last frame was drawn (rAF time), and how long it took, in ms. */
  private drawn = 0;
  private drawCost = 0;
  private dimmed = false;
  /** Bumped when the models or their pieces change, so what is worked out from them is redone. */
  private version = 0;
  /** The older side's unchanged meshes, hidden while its changes are drawn ("merged"). */
  private same: { version: number; comparison: Comparison; meshes: Object3D[] } | null = null;

  constructor(canvas: HTMLCanvasElement) {
    const r = (this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true }));
    r.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    r.setClearColor(0, 0);
    r.toneMapping = ACESFilmicToneMapping;
    r.shadowMap.enabled = true;
    r.shadowMap.type = PCFSoftShadowMap;
    // Shadows come from the light, not the camera: redrawn only when the models change or move.
    r.shadowMap.autoUpdate = false;
    const pmrem = new PMREMGenerator(r);
    const room = new RoomEnvironment();
    this.env = pmrem.fromScene(room, 0.04).texture;
    room.dispose();
    pmrem.dispose();
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.addEventListener("change", () => (this.dirty = true));
    this.controls.addEventListener("start", () => (this.cameraMoved = true));
    this.loop = this.loop.bind(this);
    this.raf = requestAnimationFrame(this.loop);
    textureEvents.addEventListener("load", this.redraw);
  }

  /** A texture arrived after the last frame: draw again so it shows. */
  private readonly redraw = () => {
    this.dirty = true;
  };

  /** Shows `after`, and `before` beside it when given; frames both. */
  setModels(before: LoadedModel | null, after: LoadedModel) {
    for (const s of this.sides) {
      if (s) {
        this.release(s);
      }
    }
    const box = new Box3().setFromObject(after.root);
    if (before) {
      box.union(new Box3().setFromObject(before.root));
    }
    this.sides = [before ? this.side(before, box) : null, this.side(after, box)];
    this.complete = false;
    this.version++;
    this.comparison = this.compareSides(before?.root ?? null, after.root);
    this.applyShading();
    this.frameAll();
    this.setClip(this.clips()[0]?.name ?? null);
    this.shadowsStale = true;
    this.compile();
  }

  /** Shows nothing: the next file couldn't load, so the last one mustn't stay on screen. */
  clear() {
    for (const s of this.sides) {
      if (s) {
        this.release(s);
      }
    }
    this.sides = [null, null];
    this.comparison = { status: new Map(), changes: [] };
    this.version++;
    this.renderer.clear();
  }

  /** A piece of a model still loading arrived: draw it with the current shading. */
  added(obj: Object3D) {
    obj.traverse((o) => {
      const mesh = o as Mesh;
      if (!mesh.isMesh) {
        return;
      }
      o.castShadow = true;
      o.userData.lit ??= mesh.material;
      if (this.shading !== "textured" && !o.userData.missing) {
        mesh.material = this.kit.clay("same");
      }
    });
    this.trickled = true;
    this.version++;
  }

  /**
   * The models shown while loading are complete: compares them, fits floor and
   * light to their real size and picks up their clips. The camera stays put; the
   * viewer frames everything again when the user hasn't moved it (`cameraMoved`).
   */
  refresh() {
    const [before, after] = this.sides;
    if (!after) {
      return;
    }
    const box = new Box3().setFromObject(after.model.root);
    if (before) {
      box.union(new Box3().setFromObject(before.model.root));
    }
    for (const s of this.sides) {
      if (s) {
        this.fit(s, box);
      }
    }
    this.comparison = this.compareSides(before?.model.root ?? null, after.model.root);
    this.complete = true;
    this.applyShading();
    const keep = this.clips().some((c) => c.name === this.clip);
    this.setClip(keep ? this.clip : (this.clips()[0]?.name ?? null));
    this.shadowsStale = true;
    this.compile();
  }

  /** Links the sides' shaders in parallel before drawing them: a big scene would block a frame for it. */
  private compile() {
    const sides = this.sides.filter((s): s is Side => !!s);
    const compiling = (this.compiling = Promise.all(sides.map((s) => this.renderer.compileAsync(s.scene, this.camera))).then(
      () => {
        if (this.compiling === compiling) {
          this.compiling = null;
          this.dirty = true;
        }
      },
      (e) => {
        console.error("Shader compile failed", e);
        this.compiling = null;
        this.dirty = true;
      },
    ));
  }

  get single() {
    return !this.sides[0];
  }

  clips() {
    return this.sides[1]?.model.clips ?? [];
  }

  duration() {
    return this.clips().find((c) => c.name === this.clip)?.duration ?? 0;
  }

  setClip(name: string | null) {
    this.clip = name;
    this.time = 0;
    for (const s of this.sides) {
      if (!s) {
        continue;
      }
      s.mixer.stopAllAction();
      const c = s.model.clips.find((x) => x.name === name);
      if (c) {
        s.mixer.clipAction(c, s.model.clipRoots?.get(c)).play();
      }
      s.mixer.setTime(0);
    }
    this.shadowsStale = true;
    this.dirty = true;
  }

  seek(time: number) {
    this.time = time;
    for (const s of this.sides) {
      s?.mixer.setTime(time);
    }
    this.shadowsStale = true;
    this.dirty = true;
  }

  private compareSides(before: Object3D | null, after: Object3D): Comparison {
    if (this.sources) {
      return bySource(after, this.sources);
    }
    return before ? compare(before, after) : { status: new Map(), changes: [] };
  }

  /** Tints the shown model by merge source (`null`: compare the sides as usual). */
  setSources(sources: Map<string, MergeSource> | null) {
    this.sources = sources;
    const [before, after] = this.sides;
    if (after) {
      this.comparison = this.compareSides(before?.model.root ?? null, after.model.root);
      this.applyShading();
    }
  }

  setShading(shading: Shading) {
    // Re-shading re-instances the whole model: only when it changes.
    if (shading === this.shading) {
      return;
    }
    this.shading = shading;
    this.applyShading();
  }

  /** In the Changes shading, fades what didn't change ("Changed only"). */
  setDimUnchanged(dim: boolean) {
    if (dim === this.dimmed) {
      return;
    }
    this.dimmed = dim;
    this.kit.dimSame(dim);
    this.dirty = true;
  }

  setOnion(opacity: number) {
    this.kit.ghost.opacity = opacity;
    this.dirty = true;
  }

  invalidate() {
    this.dirty = true;
  }

  /** Pauses drawing while the viewer is off screen. */
  setActive(active: boolean) {
    this.active = active;
    this.dirty = true;
  }

  resize(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.renderer.setSize(width, height, false);
    this.dirty = true;
  }

  frameAll() {
    const box = new Box3();
    for (const s of this.sides) {
      if (s) {
        box.expandByObject(s.model.root);
      }
    }
    if (box.isEmpty()) {
      return;
    }
    const sphere = box.getBoundingSphere(new Sphere());
    const cam = framingCamera(sphere, this.viewAspect());
    const dist = cam.distanceTo(sphere.center);
    this.controls.target.copy(sphere.center);
    this.camera.position.copy(cam);
    this.camera.near = Math.max(dist / 1000, 0.001);
    this.camera.far = dist * 100;
    this.camera.updateProjectionMatrix();
    this.fly = null;
    this.cameraMoved = false;
    this.dirty = true;
  }

  /** Width / height of the view one side is drawn in: half the canvas side by side. */
  private viewAspect(): number {
    if (!this.width || !this.height) {
      return 1;
    }
    const w = this.sides[0] && this.mode === "side" ? Math.floor(this.width / 2) : this.width;
    return w / this.height;
  }

  /** Flies the camera to `obj` over a quarter second. */
  frame(obj: Object3D) {
    const box = new Box3().setFromObject(obj);
    if (box.isEmpty()) {
      return;
    }
    const sphere = box.getBoundingSphere(new Sphere());
    const dist = fitDistance(Math.max(sphere.radius, 0.05), this.viewAspect()) * 1.4;
    const dir = this.camera.position.clone().sub(this.controls.target).normalize();
    this.cameraMoved = true;
    this.fly = {
      from: this.controls.target.clone(),
      to: sphere.center,
      camFrom: this.camera.position.clone(),
      camTo: sphere.center.clone().addScaledVector(dir, dist),
      k: 0,
    };
  }

  /** Frames what is under canvas point (x, y) in CSS pixels, or everything. */
  pick(x: number, y: number) {
    let side = this.sides[1];
    let u = x / this.width;
    if (this.sides[0] && this.mode === "side") {
      if (u < 0.5) {
        side = this.sides[0];
        u *= 2;
      } else {
        u = (u - 0.5) * 2;
      }
    } else if (this.sides[0] && this.mode === "swipe" && u < this.split) {
      side = this.sides[0];
    }
    if (!side) {
      return;
    }
    const ray = new Raycaster();
    ray.setFromCamera(new Vector2(u * 2 - 1, -(y / this.height) * 2 + 1), this.camera);
    const hit = ray.intersectObject(side.model.root, true)[0];
    if (hit) {
      this.frame(hit.object);
    } else {
      this.frameAll();
    }
  }

  dispose() {
    textureEvents.removeEventListener("load", this.redraw);
    cancelAnimationFrame(this.raf);
    for (const s of this.sides) {
      if (s) {
        this.release(s);
      }
    }
    this.controls.dispose();
    this.kit.dispose();
    this.env.dispose();
    this.renderer.dispose();
    // Browsers cap live WebGL contexts; a remounted viewer would otherwise wait on GC for this one.
    this.renderer.forceContextLoss();
  }

  /** Frees a side's GPU resources; the shading kit's materials and what the model cache holds stay. */
  private release(side: Side) {
    side.mixer.stopAllAction();
    side.instanced?.undo();
    const textures = new Set<{ dispose(): void }>();
    side.scene.traverse((o) => {
      // The rig's key light owns a shadow map render target.
      (o as DirectionalLight).shadow?.dispose();
      const mesh = o as Mesh;
      if (mesh.geometry && !mesh.geometry.userData.shared) {
        mesh.geometry.dispose();
      }
      const mats = [o.userData.lit ?? mesh.material].flat().filter(Boolean) as Material[];
      for (const m of mats) {
        if (m.userData.shared) {
          continue;
        }
        for (const v of Object.values(m)) {
          if (v && typeof v === "object" && (v as { isTexture?: boolean }).isTexture) {
            textures.add(v as { dispose(): void });
          }
        }
        m.dispose();
      }
    });
    for (const t of textures) {
      t.dispose();
    }
  }

  /** One version's scene; `box` spans both versions, so floor, grid and light match across sides. */
  private side(model: LoadedModel, box: Box3): Side {
    const scene = new Scene();
    scene.environment = this.env;
    scene.environmentIntensity = 0.55;
    const side: Side = { scene, rig: new Group(), helpers: new Group(), instanced: null, model, mixer: new AnimationMixer(model.root) };
    scene.add(side.rig, model.root);
    this.fit(side, box);
    return side;
  }

  /** (Re)builds a side's lights, floor and grid for `box`, and lets its meshes cast shadows. */
  private fit(side: Side, box: Box3) {
    side.rig.traverse((o) => {
      const mesh = o as Mesh;
      mesh.geometry?.dispose();
      (mesh.material as Material | undefined)?.dispose();
      (o as DirectionalLight).shadow?.dispose();
    });
    side.rig.clear();
    const size = box.isEmpty() ? 1 : box.getSize(new Vector3()).length();
    const floorY = box.isEmpty() ? 0 : box.min.y;
    const hemi = new HemisphereLight("#ffffff", "#5a6070", 0.5);
    const key = new DirectionalLight("#fff6ea", 2.4);
    key.position.set(size, size * 1.6, size * 0.8);
    key.castShadow = true;
    key.shadow.mapSize.set(1024, 1024);
    Object.assign(key.shadow.camera, { left: -size, right: size, top: size, bottom: -size, near: size * 0.05, far: size * 5 });
    key.shadow.bias = -0.0006;
    const helpers = new Group();
    const floor = new Mesh(new PlaneGeometry(size * 8, size * 8).rotateX(-Math.PI / 2), new ShadowMaterial({ opacity: 0.3 }));
    floor.position.y = floorY;
    floor.receiveShadow = true;
    const grid = new GridHelper(size * 4, 40, "#8a919c", "#8a919c");
    (grid.material as Material).transparent = true;
    (grid.material as Material).opacity = 0.14;
    grid.position.y = floorY;
    helpers.add(floor, grid);
    side.model.root.traverse((o) => {
      if ((o as Mesh).isMesh) {
        o.castShadow = true;
      }
    });
    side.rig.add(hemi, key, helpers);
    side.helpers = helpers;
  }

  private applyShading() {
    for (const s of this.sides) {
      s?.model.root.traverse((o) => {
        const mesh = o as Mesh;
        if (!mesh.isMesh) {
          return;
        }
        o.userData.lit ??= mesh.material;
        const status: Status = this.comparison.status.get(o) ?? "same";
        mesh.material = this.shading === "textured" || o.userData.missing ? o.userData.lit : this.kit.clay(status);
      });
    }
    this.reinstance();
    this.dirty = true;
  }

  /**
   * Draws each complete side's unchanged, unanimated meshes instanced, with the
   * materials they show now: thousands of draw calls become one per kind of mesh.
   */
  private reinstance() {
    for (const s of this.sides) {
      if (!s) {
        continue;
      }
      s.instanced?.undo();
      s.instanced = null;
      if (!this.complete) {
        continue;
      }
      const animated = new Set<Object3D>();
      for (const root of s.model.clipRoots?.values() ?? []) {
        root.traverse((o) => animated.add(o));
      }
      s.instanced = instance(
        s.model.root,
        (m) => !animated.has(m) && !m.userData.missing && (this.comparison.status.get(m) ?? "same") === "same",
      );
      s.scene.add(s.instanced.group);
    }
    this.shadowsStale = true;
  }

  private loop(now: number) {
    this.raf = requestAnimationFrame(this.loop);
    const dt = Math.min((now - (this.last || now)) / 1000, 0.1);
    this.last = now;
    if (!this.active) {
      return;
    }
    if (this.fly) {
      const f = this.fly;
      f.k = Math.min(1, f.k + dt * 4);
      const e = 1 - (1 - f.k) ** 3;
      this.controls.target.lerpVectors(f.from, f.to, e);
      this.camera.position.lerpVectors(f.camFrom, f.camTo, e);
      if (f.k === 1) {
        this.fly = null;
      }
      this.dirty = true;
    }
    this.controls.update(dt);
    const dur = this.duration();
    if (this.playing && dur > 0) {
      this.seek((this.time + dt) % dur);
      this.ontime(this.time, dur);
    }
    // Pieces still arriving redraw now and then, spaced so drawing takes a tenth of the time at most:
    // a frame for each would slow their loading. Their shadows wait until the model is complete.
    if (this.trickled && now - this.drawn > Math.min(Math.max(TRICKLE_MS, this.drawCost * 10), TRICKLE_MAX_MS)) {
      this.dirty = true;
    }
    if (this.dirty && !this.compiling) {
      this.dirty = false;
      this.trickled = false;
      this.drawn = now;
      const start = performance.now();
      this.render();
      this.drawCost = performance.now() - start;
    }
  }

  private overlay(side: Side, mat: Material) {
    const r = this.renderer;
    r.shadowMap.needsUpdate = false;
    side.helpers.visible = false;
    side.scene.overrideMaterial = mat;
    r.autoClear = false;
    r.render(side.scene, this.camera);
    r.autoClear = true;
    side.scene.overrideMaterial = null;
    side.helpers.visible = true;
  }

  /** Draws only `side`'s changed or removed meshes, as amber wire ghosts. */
  private overlayChanged(side: Side) {
    if (this.same?.version !== this.version || this.same.comparison !== this.comparison) {
      const meshes: Object3D[] = [];
      side.model.root.traverse((o) => {
        if ((o as Mesh).isMesh && (this.comparison.status.get(o) ?? "same") === "same") {
          meshes.push(o);
        }
      });
      this.same = { version: this.version, comparison: this.comparison, meshes };
    }
    const hidden = this.same.meshes.filter((o) => o.visible);
    for (const o of hidden) {
      o.visible = false;
    }
    // Instanced meshes are all unchanged ones.
    const instanced = side.instanced?.group;
    if (instanced) {
      instanced.visible = false;
    }
    this.overlay(side, this.kit.ghostWire);
    if (instanced) {
      instanced.visible = true;
    }
    for (const o of hidden) {
      o.visible = true;
    }
  }

  /** Renders `side`'s scene, its shadow map too when the models changed since it was drawn. */
  private renderSide(side: Side, shadows: boolean) {
    this.renderer.shadowMap.needsUpdate = shadows;
    this.renderer.render(side.scene, this.camera);
  }

  private draw(side: Side, x: number, w: number, shadows: boolean) {
    const r = this.renderer;
    r.setViewport(x, 0, w, this.height);
    r.setScissor(x, 0, w, this.height);
    this.renderSide(side, shadows);
    if (this.wire) {
      this.overlay(side, this.kit.wire);
    }
  }

  private render() {
    const { width: w, height: h } = this;
    const [before, after] = this.sides;
    if (!w || !h || !after) {
      return;
    }
    const r = this.renderer;
    const shadows = this.shadowsStale;
    this.shadowsStale = false;
    r.setScissorTest(true);
    if (before && this.mode === "side") {
      const half = Math.floor(w / 2);
      this.camera.aspect = half / h;
      this.camera.updateProjectionMatrix();
      this.draw(before, 0, half, shadows);
      this.draw(after, half, w - half, shadows);
    } else {
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
      if (before && this.mode === "swipe") {
        const x = Math.round(w * this.split);
        this.draw(before, 0, w, shadows);
        r.setViewport(0, 0, w, h);
        r.setScissor(x, 0, w - x, h);
        this.renderSide(after, shadows);
        if (this.wire) {
          this.overlay(after, this.kit.wire);
        }
      } else {
        this.draw(after, 0, w, shadows);
        if (before && this.mode === "onion") {
          this.overlay(before, this.kit.ghost);
        } else if (before && this.mode === "merged") {
          this.overlayChanged(before);
        }
      }
    }
    r.setScissorTest(false);
  }
}
