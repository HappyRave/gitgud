import {
  ACESFilmicToneMapping,
  AnimationMixer,
  Box3,
  DirectionalLight,
  GridHelper,
  Group,
  HemisphereLight,
  MathUtils,
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
import { type Comparison, compare, type Status } from "./compare";
import type { LoadedModel } from "./load";
import { ShadingKit } from "./materials";

/** "merged": the newer version only, with the older one's changed objects as amber wire ghosts. */
export type CompareMode = "side" | "swipe" | "onion" | "merged";
export type Shading = "textured" | "changes";

interface Side {
  scene: Scene;
  helpers: Group;
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

const FOV = 32;

/**
 * Draws one or two versions of a model into one canvas: side by side, swiped
 * or onion-skinned, with a shared camera and a shared animation clock.
 */
export class ModelStage {
  readonly renderer: WebGLRenderer;
  readonly camera = new PerspectiveCamera(FOV, 1, 0.01, 1000);
  readonly controls: OrbitControls;
  comparison: Comparison = { status: new Map(), changes: [] };
  mode: CompareMode = "side";
  shading: Shading = "textured";
  wire = false;
  split = 0.5;
  playing = false;
  time = 0;
  clip: string | null = null;
  /** Called every frame the clock moves, with the time and clip length. */
  ontime: (time: number, duration: number) => void = () => {};

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

  constructor(canvas: HTMLCanvasElement) {
    const r = (this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true }));
    r.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    r.setClearColor(0, 0);
    r.toneMapping = ACESFilmicToneMapping;
    r.shadowMap.enabled = true;
    r.shadowMap.type = PCFSoftShadowMap;
    const pmrem = new PMREMGenerator(r);
    this.env = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    pmrem.dispose();
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.addEventListener("change", () => (this.dirty = true));
    this.loop = this.loop.bind(this);
    this.raf = requestAnimationFrame(this.loop);
  }

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
    this.comparison = before ? compare(before.root, after.root) : { status: new Map(), changes: [] };
    this.applyShading();
    this.frameAll();
    this.setClip(this.clips()[0]?.name ?? null);
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
        s.mixer.clipAction(c).play();
      }
      s.mixer.setTime(0);
    }
    this.dirty = true;
  }

  seek(time: number) {
    this.time = time;
    for (const s of this.sides) {
      s?.mixer.setTime(time);
    }
    this.dirty = true;
  }

  setShading(shading: Shading) {
    this.shading = shading;
    this.applyShading();
  }

  /** In the Changes shading, fades what didn't change ("Changed only"). */
  setDimUnchanged(dim: boolean) {
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
    const dist = sphere.radius / Math.sin(MathUtils.degToRad(FOV / 2)) * 1.15;
    const dir = new Vector3(0.62, 0.38, 0.69).normalize();
    this.controls.target.copy(sphere.center);
    this.camera.position.copy(sphere.center).addScaledVector(dir, dist);
    this.camera.near = Math.max(dist / 1000, 0.001);
    this.camera.far = dist * 100;
    this.camera.updateProjectionMatrix();
    this.fly = null;
    this.dirty = true;
  }

  /** Flies the camera to `obj` over a quarter second. */
  frame(obj: Object3D) {
    const box = new Box3().setFromObject(obj);
    if (box.isEmpty()) {
      return;
    }
    const sphere = box.getBoundingSphere(new Sphere());
    const dist = Math.max(sphere.radius, 0.05) / Math.sin(MathUtils.degToRad(FOV / 2)) * 1.4;
    const dir = this.camera.position.clone().sub(this.controls.target).normalize();
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
  }

  /** Frees a side's GPU resources; the shading kit's materials and what the model cache holds stay. */
  private release(side: Side) {
    side.mixer.stopAllAction();
    const textures = new Set<{ dispose(): void }>();
    side.scene.traverse((o) => {
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
    model.root.traverse((o) => {
      if ((o as Mesh).isMesh) {
        o.castShadow = true;
      }
    });
    scene.add(hemi, key, helpers, model.root);
    return { scene, helpers, model, mixer: new AnimationMixer(model.root) };
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
    this.dirty = true;
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
    if (this.dirty) {
      this.dirty = false;
      this.render();
    }
  }

  private overlay(side: Side, mat: Material) {
    const r = this.renderer;
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
    const hidden: Object3D[] = [];
    side.model.root.traverse((o) => {
      if ((o as Mesh).isMesh && o.visible && (this.comparison.status.get(o) ?? "same") === "same") {
        o.visible = false;
        hidden.push(o);
      }
    });
    this.overlay(side, this.kit.ghostWire);
    for (const o of hidden) {
      o.visible = true;
    }
  }

  private draw(side: Side, x: number, w: number) {
    const r = this.renderer;
    r.setViewport(x, 0, w, this.height);
    r.setScissor(x, 0, w, this.height);
    r.render(side.scene, this.camera);
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
    r.setScissorTest(true);
    if (before && this.mode === "side") {
      const half = Math.floor(w / 2);
      this.camera.aspect = half / h;
      this.camera.updateProjectionMatrix();
      this.draw(before, 0, half);
      this.draw(after, half, w - half);
    } else {
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
      if (before && this.mode === "swipe") {
        const x = Math.round(w * this.split);
        this.draw(before, 0, w);
        r.setViewport(0, 0, w, h);
        r.setScissor(x, 0, w - x, h);
        r.render(after.scene, this.camera);
        if (this.wire) {
          this.overlay(after, this.kit.wire);
        }
      } else {
        this.draw(after, 0, w);
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
