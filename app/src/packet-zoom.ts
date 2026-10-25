// Zoom for the build packet on screen: pinch, Cmd/Ctrl with + - 0, Cmd/Ctrl
// with the scroll wheel, and the bar's buttons. The zoom is a screen-only style
// inside the preview, so printing and the downloaded page stay full size.

const KEY = "sticks.packetZoom";
const MIN = 0.25;
const MAX = 4;
const STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];

const clamp = (z: number) => Math.min(MAX, Math.max(MIN, z));

export interface PacketZoom {
  /** Steps to the next zoom up (+1) or down (-1). */
  step(dir: 1 | -1): void;
  set(zoom: number): void;
}

/**
 * Wires zoom to the preview's iframe and the bar's label. The frame's page is
 * replaced on every Print, so the zoom goes back in on each load.
 */
export function packetZoom(frame: HTMLIFrameElement, label: HTMLElement, isOpen: () => boolean): PacketZoom {
  let zoom = 1;
  try {
    zoom = clamp(Number(localStorage.getItem(KEY)) || 1);
  } catch {
    // No storage: start at full size.
  }

  const apply = (keepX?: number, keepY?: number) => {
    label.textContent = `${Math.round(zoom * 100)}%`;
    const doc = frame.contentDocument;
    if (!doc?.head) return;
    let style = doc.getElementById("sticks-zoom");
    if (!style) {
      style = doc.createElement("style");
      style.id = "sticks-zoom";
      doc.head.append(style);
    }
    const win = frame.contentWindow!;
    const old = Number(style.dataset.zoom) || 1;
    // `zoom` rather than a transform so the page scrolls to its zoomed size.
    style.textContent = `@media screen { body { zoom: ${zoom}; } }`;
    style.dataset.zoom = String(zoom);
    const x = keepX ?? win.innerWidth / 2;
    const y = keepY ?? win.innerHeight / 2;
    const k = zoom / old;
    win.scrollTo((win.scrollX + x) * k - x, (win.scrollY + y) * k - y);
  };

  const set = (z: number, x?: number, y?: number) => {
    zoom = clamp(z);
    try {
      localStorage.setItem(KEY, String(zoom));
    } catch {
      // Remembering the zoom is only a convenience.
    }
    apply(x, y);
  };

  const step = (dir: 1 | -1) => {
    const next = dir > 0 ? STEPS.find((s) => s > zoom + 0.001) : [...STEPS].reverse().find((s) => s < zoom - 0.001);
    set(next ?? zoom);
  };

  /** Cmd/Ctrl with + - 0, from the app or from inside the preview. */
  const onKey = (e: KeyboardEvent) => {
    if (!isOpen() || !(e.metaKey || e.ctrlKey) || e.altKey) return;
    if (e.key === "=" || e.key === "+") step(1);
    else if (e.key === "-" || e.key === "_") step(-1);
    else if (e.key === "0") set(1);
    else return;
    e.preventDefault();
  };
  document.addEventListener("keydown", onKey);

  frame.addEventListener("load", () => {
    const win = frame.contentWindow;
    if (!win) return;
    apply(0, 0);
    win.document.addEventListener("keydown", onKey);
    // Pinch on a trackpad comes as a wheel with ctrlKey in Chrome and Firefox,
    // and Cmd/Ctrl with a mouse wheel zooms too.
    win.addEventListener(
      "wheel",
      (e) => {
        if (!(e.ctrlKey || e.metaKey)) return;
        e.preventDefault();
        const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
        set(zoom * Math.exp(-dy / 400), e.clientX, e.clientY);
      },
      { passive: false },
    );
    // Safari and the Mac app's WebKit send pinches as gesture events instead.
    let start = zoom;
    let at = { x: 0, y: 0 };
    win.addEventListener("gesturestart", (e) => {
      e.preventDefault();
      start = zoom;
      const g = e as unknown as { clientX: number; clientY: number };
      at = { x: g.clientX, y: g.clientY };
    });
    win.addEventListener("gesturechange", (e) => {
      e.preventDefault();
      set(start * (e as unknown as { scale: number }).scale, at.x, at.y);
    });
    win.addEventListener("gestureend", (e) => e.preventDefault());
  });

  label.textContent = `${Math.round(zoom * 100)}%`;
  return {
    step,
    set,
  };
}
