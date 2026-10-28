// Zoom for the flat drawings (walls, floor, roof, sheathing...), with the same
// gestures as the build packet: pinch, Cmd/Ctrl with + - 0, Cmd/Ctrl with the
// scroll wheel, and the corner buttons. Zoomed in, the drawing scrolls, and
// dragging it moves it around.

const KEY = "sticks.drawingZoom";
/** 100% is the drawing fitted to the stage, so there's nothing smaller to show. */
const MIN = 1;
const MAX = 8;
const STEPS = [1, 1.25, 1.5, 2, 2.5, 3, 4, 5, 6, 8];
/** Pixels the pointer must travel before a press becomes a pan. */
const PAN_START = 4;

const clamp = (z: number) => Math.min(MAX, Math.max(MIN, z));

export interface DrawingZoom {
  /** The shown drawing's zoom, 1 when it fits the stage. */
  readonly value: number;
  /** Steps to the next zoom up (+1) or down (-1). */
  step(dir: 1 | -1): void;
  set(zoom: number): void;
  /** Switches to a drawing's own zoom and place, before it's drawn. */
  view(key: string): void;
  /** Puts the drawing back where it was scrolled to, once it's drawn. */
  drawn(): void;
}

/**
 * Wires zoom to the scrolling box the flat drawings sit in. `redraw` sizes the
 * shown drawing to the zoom. Each drawing keeps its own zoom; the place it was
 * scrolled to is kept until the app closes.
 */
export function drawingZoom(
  scroller: HTMLElement,
  label: HTMLElement,
  isActive: () => boolean,
  redraw: () => void,
): DrawingZoom {
  let saved: Record<string, number> = {};
  try {
    saved = JSON.parse(localStorage.getItem(KEY) ?? "{}") ?? {};
  } catch {
    // No storage: every drawing starts fitted.
  }
  const places = new Map<string, { left: number; top: number }>();
  let key = "";
  let zoom = 1;
  let pending: { left: number; top: number } | null = null;

  const show = () => {
    label.textContent = `${Math.round(zoom * 100)}%`;
    scroller.classList.toggle("zoomed", zoom > 1);
  };

  const set = (z: number, x?: number, y?: number) => {
    const old = zoom;
    zoom = clamp(z);
    if (zoom === old) return;
    if (zoom === 1) delete saved[key];
    else saved[key] = zoom;
    try {
      localStorage.setItem(KEY, JSON.stringify(saved));
    } catch {
      // Remembering the zoom is only a convenience.
    }
    show();
    // Keep the point under the pointer, or the middle, where it is.
    const px = x ?? scroller.clientWidth / 2;
    const py = y ?? scroller.clientHeight / 2;
    const k = zoom / old;
    const left = (scroller.scrollLeft + px) * k - px;
    const top = (scroller.scrollTop + py) * k - py;
    redraw();
    scroller.scrollTo(left, top);
  };

  const step = (dir: 1 | -1) => {
    const next = dir > 0 ? STEPS.find((s) => s > zoom + 0.001) : [...STEPS].reverse().find((s) => s < zoom - 0.001);
    set(next ?? zoom);
  };

  /** A point in the scroller's own terms. */
  const at = (clientX: number, clientY: number) => {
    const r = scroller.getBoundingClientRect();
    return { x: clientX - r.left, y: clientY - r.top };
  };

  document.addEventListener("keydown", (e) => {
    if (!isActive() || !(e.metaKey || e.ctrlKey) || e.altKey || document.querySelector("dialog[open]")) return;
    if (e.key === "=" || e.key === "+") step(1);
    else if (e.key === "-" || e.key === "_") step(-1);
    else if (e.key === "0") set(1);
    else return;
    e.preventDefault();
  });

  // Pinch on a trackpad comes as a wheel with ctrlKey in Chrome and Firefox,
  // and Cmd/Ctrl with a mouse wheel zooms too. A plain wheel scrolls.
  scroller.addEventListener(
    "wheel",
    (e) => {
      if (!isActive() || !(e.ctrlKey || e.metaKey)) return;
      e.preventDefault();
      const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      const p = at(e.clientX, e.clientY);
      set(zoom * Math.exp(-dy / 400), p.x, p.y);
    },
    { passive: false },
  );

  // Safari and the Mac app's WebKit send pinches as gesture events instead.
  let start = zoom;
  let pinch = { x: 0, y: 0 };
  scroller.addEventListener("gesturestart", (e) => {
    if (!isActive()) return;
    e.preventDefault();
    start = zoom;
    const g = e as unknown as { clientX: number; clientY: number };
    pinch = at(g.clientX, g.clientY);
  });
  scroller.addEventListener("gesturechange", (e) => {
    if (!isActive()) return;
    e.preventDefault();
    set(start * (e as unknown as { scale: number }).scale, pinch.x, pinch.y);
  });
  scroller.addEventListener("gestureend", (e) => e.preventDefault());

  // Dragging the drawing moves it around, from anywhere but an opening (which
  // drags the opening). The middle button pans from anywhere.
  let pan: { pointer: number; x: number; y: number; left: number; top: number; moved: boolean } | null = null;
  scroller.addEventListener("pointerdown", (e) => {
    if (!isActive() || e.pointerType === "touch") return;
    if (e.button === 0 && (e.target as Element).closest(".opening-handle")) return;
    if (e.button !== 0 && e.button !== 1) return;
    const scrolls = scroller.scrollWidth > scroller.clientWidth || scroller.scrollHeight > scroller.clientHeight;
    if (!scrolls) return;
    if (e.button === 1) e.preventDefault();
    pan = { pointer: e.pointerId, x: e.clientX, y: e.clientY, left: scroller.scrollLeft, top: scroller.scrollTop, moved: false };
  });
  scroller.addEventListener("pointermove", (e) => {
    if (!pan || e.pointerId !== pan.pointer) return;
    const dx = e.clientX - pan.x;
    const dy = e.clientY - pan.y;
    if (!pan.moved) {
      if (Math.hypot(dx, dy) < PAN_START) return;
      pan.moved = true;
      scroller.setPointerCapture(e.pointerId);
      scroller.classList.add("panning");
    }
    scroller.scrollTo(pan.left - dx, pan.top - dy);
  });
  const endPan = (e: PointerEvent) => {
    if (!pan || e.pointerId !== pan.pointer) return;
    const moved = pan.moved;
    pan = null;
    scroller.classList.remove("panning");
    if (scroller.hasPointerCapture(e.pointerId)) scroller.releasePointerCapture(e.pointerId);
    if (!moved) return;
    // Letting go after a pan isn't a click, so a picked board stays picked.
    const swallow = (c: Event) => c.stopPropagation();
    scroller.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => scroller.removeEventListener("click", swallow, { capture: true }), 0);
  };
  scroller.addEventListener("pointerup", endPan);
  scroller.addEventListener("pointercancel", endPan);

  show();
  return {
    get value() {
      return zoom;
    },
    step,
    set,
    view(next) {
      if (next === key) return;
      if (key) places.set(key, { left: scroller.scrollLeft, top: scroller.scrollTop });
      key = next;
      zoom = clamp(saved[key] ?? 1);
      pending = places.get(key) ?? { left: 0, top: 0 };
      show();
    },
    drawn() {
      if (!pending) return;
      scroller.scrollTo(pending.left, pending.top);
      pending = null;
    },
  };
}
