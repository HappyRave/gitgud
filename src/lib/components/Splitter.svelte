<script lang="ts">
  import { onDestroy } from "svelte";

  let {
    orientation,
    label,
    class: className = "",
    onstart,
    ondrag,
    onstep,
    onreset,
  }: {
    orientation: "vertical" | "horizontal";
    label: string;
    class?: string;
    onstart?: () => void;
    ondrag: (totalDeltaPx: number) => void;
    onstep?: (deltaPx: number) => void;
    onreset?: () => void;
  } = $props();

  let dragging = $state(false);
  let cleanup: (() => void) | null = null;

  function stopDrag() {
    cleanup?.();
    cleanup = null;
  }

  onDestroy(() => stopDrag());

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    stopDrag();

    const startPos = orientation === "vertical" ? event.clientX : event.clientY;
    const pointerId = event.pointerId;
    const target = event.currentTarget as HTMLElement;
    const prevCursor = document.body.style.cursor;
    const prevUserSelect = document.body.style.userSelect;

    dragging = true;
    onstart?.();
    try {
      target.setPointerCapture?.(pointerId);
    } catch {
      // Synthetic pointer events in tests may omit pointer capture registration.
    }
    document.body.style.cursor = orientation === "vertical" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";

    const onMove = (e: PointerEvent) => {
      const pos = orientation === "vertical" ? e.clientX : e.clientY;
      ondrag(pos - startPos);
    };
    const onEnd = () => stopDrag();

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onEnd);
    window.addEventListener("pointercancel", onEnd);

    cleanup = () => {
      dragging = false;
      document.body.style.cursor = prevCursor;
      document.body.style.userSelect = prevUserSelect;
      try {
        target.releasePointerCapture?.(pointerId);
      } catch {
        // Pointer capture may already be released.
      }
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onEnd);
      window.removeEventListener("pointercancel", onEnd);
    };
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Home") {
      event.preventDefault();
      onreset?.();
      return;
    }
    if (!onstep) {
      return;
    }
    if (orientation === "vertical") {
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        onstep(-16);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        onstep(16);
      }
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      onstep(-16);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      onstep(16);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="splitter {orientation} {className}"
  class:dragging
  role="separator"
  aria-orientation={orientation}
  aria-label={label}
  tabindex="0"
  title="Drag to resize · Double-click to reset"
  onpointerdown={onPointerDown}
  ondblclick={() => onreset?.()}
  onkeydown={onKeydown}
></div>

<style>
  .splitter {
    position: relative;
    flex: none;
    touch-action: none;
    user-select: none;
    background: transparent;
  }
  .splitter.vertical {
    width: 5px;
    cursor: col-resize;
  }
  .splitter.horizontal {
    height: 5px;
    cursor: row-resize;
  }
  .splitter::after {
    content: "";
    position: absolute;
    background: var(--line);
    transition: background 0.1s ease;
  }
  .splitter.vertical::after {
    top: 0;
    bottom: 0;
    left: 2px;
    width: 1px;
  }
  .splitter.horizontal::after {
    left: 0;
    right: 0;
    top: 2px;
    height: 1px;
  }
  .splitter:hover::after,
  .splitter.dragging::after,
  .splitter:focus-visible::after {
    background: var(--l0);
  }
  .splitter.vertical:hover::after,
  .splitter.vertical.dragging::after,
  .splitter.vertical:focus-visible::after {
    left: 1px;
    width: 2px;
  }
  .splitter.horizontal:hover::after,
  .splitter.horizontal.dragging::after,
  .splitter.horizontal:focus-visible::after {
    top: 1px;
    height: 2px;
  }
  .splitter:focus-visible {
    outline: none;
  }
</style>
