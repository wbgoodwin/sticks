// The 3D view: every member as a solid, orbited with the mouse. Hovering a
// member names it; clicking one shows its size, length and cuts.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { ModelView, SolidKind, SolidView } from "./core";

/** Groups of solids that can be hidden to see inside. */
export type Layer = "sheathing" | "roof" | "walls" | "porch" | "subfloor" | "floor" | "deck";

export const LAYERS: [Layer, string][] = [
  ["sheathing", "Sheathing"],
  ["roof", "Roof"],
  ["walls", "Walls"],
  ["porch", "Porch"],
  ["subfloor", "Subfloor"],
  ["floor", "Floor framing"],
  ["deck", "Deck"],
];

/** Hidden until turned on, so the framing shows first. */
export const HIDDEN_LAYERS: Layer[] = ["sheathing"];

export function layerOf(s: SolidView): Layer {
  if (s.kind === "sheathing") return "sheathing";
  if (s.kind === "subfloor") return "subfloor";
  if (s.part === "floor") return "floor";
  if (s.part === "roof") return "roof";
  if (s.part === "porch" || s.part === "porch-deck") return "porch";
  if (s.part === "deck") return "deck";
  return "walls";
}

const WOOD: Partial<Record<SolidKind, number>> = {
  bottom_plate: 0xc9965a,
  top_plate: 0xc9965a,
  cap_plate: 0xc9965a,
  stud: 0xe2b77a,
  king_stud: 0xd4a25f,
  jack_stud: 0xe9c792,
  cripple: 0xefd5a8,
  header: 0xb07a3c,
  sill: 0xc48c4c,
  blocking: 0xd9b98a,
  rim_joist: 0xc9965a,
  joist: 0xe2b77a,
  beam: 0xa8743a,
  skid: 0xa8743a,
  ledger: 0xb07a3c,
  rafter: 0xdcae6e,
  fascia: 0xc48c4c,
  post: 0xa8743a,
  ridge: 0xb07a3c,
  collar_tie: 0xd4a25f,
  gable_stud: 0xe2b77a,
  decking: 0xc79a62,
  pier: 0xb5b2aa,
  subfloor: 0xd8bf86,
  sheathing: 0xd9c79c,
};

export interface ViewerEvents {
  /** The member under the pointer, for a tooltip; null when none. */
  hover(solid: SolidView | null, x: number, y: number): void;
  /** A member was clicked, or empty space (null). */
  pick(solid: SolidView | null): void;
}

export class Viewer {
  readonly canvas: HTMLCanvasElement;
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(35, 1, 1, 20000);
  private controls: OrbitControls;
  private group = new THREE.Group();
  private ground: THREE.Mesh;
  private sun = new THREE.DirectionalLight(0xffffff, 2.2);
  private materials = new Map<string, THREE.MeshStandardMaterial>();
  private edgeMaterial = new THREE.LineBasicMaterial({ color: 0x5a3d1c, transparent: true, opacity: 0.45 });
  private meshes: THREE.Mesh[] = [];
  private hidden = new Set<Layer>(HIDDEN_LAYERS);
  private selected: THREE.Mesh | null = null;
  private hovered: THREE.Mesh | null = null;
  private raycaster = new THREE.Raycaster();
  private fitted = false;
  private active = false;
  private down: { x: number; y: number } | null = null;

  constructor(
    private container: HTMLElement,
    private events: ViewerEvents,
  ) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.canvas = this.renderer.domElement;
    this.canvas.className = "view3d";
    container.prepend(this.canvas);

    THREE.Object3D.DEFAULT_UP.set(0, 0, 1);
    this.camera.up.set(0, 0, 1);
    this.controls = new OrbitControls(this.camera, this.canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.12;
    this.controls.maxPolarAngle = Math.PI * 0.495;

    this.scene.add(new THREE.HemisphereLight(0xfff8ec, 0x8a8170, 1.6));
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(2048, 2048);
    this.sun.shadow.bias = -0.0004;
    this.scene.add(this.sun, this.sun.target);

    this.ground = new THREE.Mesh(
      new THREE.CircleGeometry(1, 64),
      new THREE.ShadowMaterial({ opacity: 0.16 }),
    );
    this.ground.receiveShadow = true;
    this.scene.add(this.ground, this.group);

    this.canvas.addEventListener("pointerdown", (e) => (this.down = { x: e.clientX, y: e.clientY }));
    this.canvas.addEventListener("pointerup", (e) => {
      const d = this.down;
      this.down = null;
      if (!d || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4) return;
      this.select(this.hit(e));
      this.events.pick(this.selected ? (this.selected.userData.solid as SolidView) : null);
    });
    this.canvas.addEventListener("pointermove", (e) => {
      if (e.buttons !== 0) return;
      const mesh = this.hit(e);
      this.setHover(mesh);
      const box = this.container.getBoundingClientRect();
      this.events.hover(mesh ? (mesh.userData.solid as SolidView) : null, e.clientX - box.left, e.clientY - box.top);
    });
    this.canvas.addEventListener("pointerleave", () => {
      this.setHover(null);
      this.events.hover(null, 0, 0);
    });
    new ResizeObserver(() => this.resize()).observe(container);
  }

  /** Runs the render loop only while the view is on screen. */
  setActive(active: boolean): void {
    this.active = active;
    // three.js sets an inline display on its canvas, so `hidden` alone won't do.
    this.canvas.style.display = active ? "block" : "none";
    if (active) {
      this.resize();
      this.renderer.setAnimationLoop(() => {
        this.controls.update();
        this.renderer.render(this.scene, this.camera);
      });
    } else {
      this.renderer.setAnimationLoop(null);
    }
  }

  setModel(model: ModelView): void {
    const selectedLabel = this.selected?.userData.solid.label as string | undefined;
    for (const child of [...this.group.children]) {
      this.group.remove(child);
      child.traverse((o) => {
        if (o instanceof THREE.Mesh || o instanceof THREE.LineSegments) o.geometry.dispose();
      });
    }
    this.meshes = [];
    this.selected = null;
    this.hovered = null;
    for (const s of model.solids) {
      const mesh = this.solidMesh(s);
      this.group.add(mesh);
      this.meshes.push(mesh);
      if (s.label === selectedLabel) this.select(mesh);
    }
    this.applyLayers();
    if (model.bounds) this.frameBounds(model.bounds, !this.fitted);
    this.fitted ||= model.bounds !== null;
  }

  setLayer(layer: Layer, visible: boolean): void {
    if (visible) this.hidden.delete(layer);
    else this.hidden.add(layer);
    this.applyLayers();
  }

  /** Puts the camera back on the starting view of the whole model. */
  resetView(): void {
    this.fitted = false;
    const box = new THREE.Box3();
    for (const m of this.meshes) box.expandByObject(m);
    if (!box.isEmpty()) {
      this.frameBounds([box.min.toArray(), box.max.toArray()], true);
      this.fitted = true;
    }
  }

  clearSelection(): void {
    this.select(null);
  }

  private applyLayers(): void {
    for (const m of this.meshes) m.visible = !this.hidden.has(layerOf(m.userData.solid));
    if (this.selected && !this.selected.visible) {
      this.select(null);
      this.events.pick(null);
    }
  }

  private material(kind: SolidKind, state: "plain" | "hover" | "selected"): THREE.MeshStandardMaterial {
    const key = `${kind}|${state}`;
    let m = this.materials.get(key);
    if (!m) {
      m = new THREE.MeshStandardMaterial({
        color: WOOD[kind] ?? 0xd9b98a,
        roughness: kind === "pier" ? 0.95 : 0.8,
        metalness: 0,
      });
      if (state === "hover") m.emissive.set(0x2f6b4f).multiplyScalar(0.25);
      if (state === "selected") {
        m.color.set(0x6fbf95);
        m.emissive.set(0x2f6b4f).multiplyScalar(0.35);
      }
      this.materials.set(key, m);
    }
    return m;
  }

  private solidMesh(s: SolidView): THREE.Mesh {
    const shape = new THREE.Shape(s.profile.map(([a, b]) => new THREE.Vector2(a, b)));
    const geometry = new THREE.ExtrudeGeometry(shape, { depth: s.depth, bevelEnabled: false, curveSegments: 1 });
    const u = new THREE.Vector3(...s.u);
    const v = new THREE.Vector3(...s.v);
    const n = new THREE.Vector3().crossVectors(u, v);
    geometry.applyMatrix4(new THREE.Matrix4().makeBasis(u, v, n).setPosition(...s.origin));
    const mesh = new THREE.Mesh(geometry, this.material(s.kind, "plain"));
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.userData.solid = s;
    mesh.add(new THREE.LineSegments(new THREE.EdgesGeometry(geometry, 20), this.edgeMaterial));
    return mesh;
  }

  private select(mesh: THREE.Mesh | null): void {
    if (this.selected) this.selected.material = this.material(this.selected.userData.solid.kind, "plain");
    this.selected = mesh;
    if (mesh) mesh.material = this.material(mesh.userData.solid.kind, "selected");
  }

  private setHover(mesh: THREE.Mesh | null): void {
    if (this.hovered && this.hovered !== this.selected) {
      this.hovered.material = this.material(this.hovered.userData.solid.kind, "plain");
    }
    this.hovered = mesh;
    if (mesh && mesh !== this.selected) mesh.material = this.material(mesh.userData.solid.kind, "hover");
    this.canvas.style.cursor = mesh ? "pointer" : "";
  }

  private hit(e: PointerEvent): THREE.Mesh | null {
    const rect = this.canvas.getBoundingClientRect();
    const pointer = new THREE.Vector2(
      ((e.clientX - rect.left) / rect.width) * 2 - 1,
      -((e.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    const visible = this.meshes.filter((m) => m.visible);
    const first = this.raycaster.intersectObjects(visible, false)[0];
    return first ? (first.object as THREE.Mesh) : null;
  }

  /** Lights and grounds the model, and on the first fit aims the camera at it. */
  private frameBounds(bounds: [number[], number[]], aim: boolean): void {
    const [lo, hi] = bounds.map((b) => new THREE.Vector3(...b));
    const center = lo.clone().add(hi).multiplyScalar(0.5);
    const radius = Math.max(24, lo.distanceTo(hi) / 2);

    this.ground.position.set(center.x, center.y, lo.z - 0.5);
    this.ground.scale.setScalar(radius * 4);
    this.sun.position.copy(center).add(new THREE.Vector3(-0.6, -1, 1.4).normalize().multiplyScalar(radius * 3));
    this.sun.target.position.copy(center);
    const cam = this.sun.shadow.camera;
    cam.left = cam.bottom = -radius * 1.3;
    cam.right = cam.top = radius * 1.3;
    cam.near = radius * 0.5;
    cam.far = radius * 6;
    cam.updateProjectionMatrix();

    if (!aim) return;
    const fov = THREE.MathUtils.degToRad(this.camera.fov);
    const distance = (radius / Math.sin(fov / 2)) * 0.8;
    this.camera.position.copy(center).add(new THREE.Vector3(-0.75, -1.5, 0.75).normalize().multiplyScalar(distance));
    this.camera.near = distance / 100;
    this.camera.far = distance * 20;
    this.camera.updateProjectionMatrix();
    this.controls.target.copy(center);
    this.controls.minDistance = radius * 0.2;
    this.controls.maxDistance = distance * 4;
    this.controls.update();
  }

  private resize(): void {
    const { width, height } = this.container.getBoundingClientRect();
    if (!this.active || width === 0 || height === 0) return;
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    // Resizing clears the canvas, and this runs after the frame's render but
    // before it is painted; without drawing again the view blinks blank.
    this.renderer.render(this.scene, this.camera);
  }
}
