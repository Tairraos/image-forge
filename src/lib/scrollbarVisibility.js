const SCROLLING_CLASS = "scrollbar-active";
const NEAR_CLASS = "scrollbar-near";
const OVERLAY_CLASS = "app-scrollbar-overlay";
const HIDE_DELAY = 160;
const EDGE_DISTANCE = 12;
const MIN_THUMB_SIZE = 26;

// 统一管理原生滚动容器和 Naive UI 滚动条的悬浮显示状态。
export function installAutoHideScrollbars() {
  const naiveHideTimers = new Map();
  const nativeOverlays = new Map();
  let nearOwners = new Set();
  let pointerFrame = 0;
  let overlayFrame = 0;
  let latestPointer = null;
  const resizeObserver = new ResizeObserver(() => requestOverlayUpdate());

  function ownerFor(element) {
    const naiveOwner = element.closest(".n-scrollbar");
    if (naiveOwner) return naiveOwner;
    return element;
  }

  function isNaiveOwner(element) {
    return element.classList.contains("n-scrollbar");
  }

  function usesPersistentNativeScrollbar(element) {
    return Boolean(element.closest("[data-persistent-scrollbar]"));
  }

  function scrollAxes(element) {
    const style = window.getComputedStyle(element);
    const vertical = /(auto|scroll|overlay)/.test(style.overflowY)
      && element.scrollHeight > element.clientHeight + 1;
    const horizontal = /(auto|scroll|overlay)/.test(style.overflowX)
      && element.scrollWidth > element.clientWidth + 1;
    return { vertical, horizontal };
  }

  function createRail(orientation) {
    const rail = document.createElement("div");
    rail.className = `${OVERLAY_CLASS} ${OVERLAY_CLASS}--${orientation}`;
    const thumb = document.createElement("div");
    thumb.className = `${OVERLAY_CLASS}__thumb`;
    rail.appendChild(thumb);
    document.body.appendChild(rail);
    return { rail, thumb };
  }

  function ensureNativeOverlay(element) {
    const existing = nativeOverlays.get(element);
    if (existing) return existing;
    const vertical = createRail("vertical");
    const horizontal = createRail("horizontal");
    const state = {
      element,
      vertical,
      horizontal,
      scrolling: false,
      near: false,
      hovered: false,
      dragging: "",
      dragOffset: 0,
      hideTimer: 0,
    };
    bindRail(state, vertical, "vertical");
    bindRail(state, horizontal, "horizontal");
    nativeOverlays.set(element, state);
    resizeObserver.observe(element);
    updateNativeOverlay(state);
    return state;
  }

  function bindRail(state, parts, orientation) {
    const { rail, thumb } = parts;
    rail.addEventListener("pointerenter", () => {
      state.hovered = true;
      updateNativeVisibility(state);
    });
    rail.addEventListener("pointerleave", () => {
      state.hovered = false;
      updateNativeVisibility(state);
    });
    rail.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      state.dragging = orientation;
      state.hovered = true;
      const thumbRect = thumb.getBoundingClientRect();
      const coordinate = orientation === "vertical" ? event.clientY : event.clientX;
      const thumbStart = orientation === "vertical" ? thumbRect.top : thumbRect.left;
      const thumbSize = orientation === "vertical" ? thumbRect.height : thumbRect.width;
      state.dragOffset = event.target === thumb ? coordinate - thumbStart : thumbSize / 2;
      rail.setPointerCapture(event.pointerId);
      scrollFromPointer(state, orientation, coordinate);
      updateNativeVisibility(state);
    });
    rail.addEventListener("pointermove", (event) => {
      if (state.dragging !== orientation) return;
      const coordinate = orientation === "vertical" ? event.clientY : event.clientX;
      scrollFromPointer(state, orientation, coordinate);
    });
    const stopDragging = (event) => {
      if (state.dragging !== orientation) return;
      state.dragging = "";
      if (rail.hasPointerCapture(event.pointerId)) rail.releasePointerCapture(event.pointerId);
      updateNativeVisibility(state);
    };
    rail.addEventListener("pointerup", stopDragging);
    rail.addEventListener("pointercancel", stopDragging);
    rail.addEventListener("wheel", (event) => {
      event.preventDefault();
      if (orientation === "vertical") {
        state.element.scrollTop += event.deltaY;
      } else {
        state.element.scrollLeft += event.deltaX || event.deltaY;
      }
    }, { passive: false });
  }

  function scrollFromPointer(state, orientation, coordinate) {
    const parts = orientation === "vertical" ? state.vertical : state.horizontal;
    const railRect = parts.rail.getBoundingClientRect();
    const thumbRect = parts.thumb.getBoundingClientRect();
    const railStart = orientation === "vertical" ? railRect.top : railRect.left;
    const railSize = orientation === "vertical" ? railRect.height : railRect.width;
    const thumbSize = orientation === "vertical" ? thumbRect.height : thumbRect.width;
    const travel = Math.max(1, railSize - thumbSize);
    const offset = Math.max(0, Math.min(coordinate - railStart - state.dragOffset, travel));
    const ratio = offset / travel;
    if (orientation === "vertical") {
      state.element.scrollTop = ratio * (state.element.scrollHeight - state.element.clientHeight);
    } else {
      state.element.scrollLeft = ratio * (state.element.scrollWidth - state.element.clientWidth);
    }
    updateNativeOverlay(state);
  }

  function showWhileScrolling(element) {
    if (usesPersistentNativeScrollbar(element)) return;
    const owner = ownerFor(element);
    if (isNaiveOwner(owner)) {
      owner.classList.add(SCROLLING_CLASS);
      window.clearTimeout(naiveHideTimers.get(owner));
      naiveHideTimers.set(owner, window.setTimeout(() => {
        owner.classList.remove(SCROLLING_CLASS);
        naiveHideTimers.delete(owner);
      }, HIDE_DELAY));
      return;
    }
    const state = ensureNativeOverlay(owner);
    state.scrolling = true;
    window.clearTimeout(state.hideTimer);
    state.hideTimer = window.setTimeout(() => {
      state.scrolling = false;
      updateNativeVisibility(state);
    }, HIDE_DELAY);
    updateNativeVisibility(state);
    requestOverlayUpdate();
  }

  function onScroll(event) {
    if (!(event.target instanceof Element)) return;
    showWhileScrolling(event.target);
    requestOverlayUpdate();
  }

  function isNearScrollbar(element, point) {
    const { vertical, horizontal } = scrollAxes(element);
    if (!vertical && !horizontal) return false;
    const rect = element.getBoundingClientRect();
    const insideX = point.x >= rect.left && point.x <= rect.right;
    const insideY = point.y >= rect.top && point.y <= rect.bottom;
    const nearVertical = vertical && insideY && insideX && rect.right - point.x <= EDGE_DISTANCE;
    const nearHorizontal = horizontal && insideX && insideY && rect.bottom - point.y <= EDGE_DISTANCE;
    return nearVertical || nearHorizontal;
  }

  function collectScrollableAncestors(element, point, owners) {
    let current = element;
    while (current && current !== document.documentElement) {
      if (!usesPersistentNativeScrollbar(current) && isNearScrollbar(current, point)) {
        owners.add(ownerFor(current));
      }
      current = current.parentElement;
    }
  }

  function setOwnerNear(owner, near) {
    if (isNaiveOwner(owner)) {
      owner.classList.toggle(NEAR_CLASS, near);
      return;
    }
    const state = near ? ensureNativeOverlay(owner) : nativeOverlays.get(owner);
    if (!state) return;
    state.near = near;
    updateNativeVisibility(state);
    if (near) updateNativeOverlay(state);
  }

  function updateNearOwners() {
    pointerFrame = 0;
    if (!latestPointer) return;
    const nextOwners = new Set();
    const element = document.elementFromPoint(latestPointer.x, latestPointer.y);
    if (element) {
      collectScrollableAncestors(element, latestPointer, nextOwners);
      const naiveScrollbar = element.closest(".n-scrollbar");
      const naiveContainer = naiveScrollbar?.querySelector(":scope > .n-scrollbar-container");
      if (naiveContainer && isNearScrollbar(naiveContainer, latestPointer)) {
        nextOwners.add(naiveScrollbar);
      }
    }
    for (const owner of nearOwners) {
      if (!nextOwners.has(owner)) setOwnerNear(owner, false);
    }
    for (const owner of nextOwners) setOwnerNear(owner, true);
    nearOwners = nextOwners;
  }

  function onPointerMove(event) {
    latestPointer = { x: event.clientX, y: event.clientY };
    if (!pointerFrame) pointerFrame = window.requestAnimationFrame(updateNearOwners);
  }

  function clearNearOwners() {
    latestPointer = null;
    for (const owner of nearOwners) setOwnerNear(owner, false);
    nearOwners.clear();
  }

  function updateNativeVisibility(state) {
    const visible = state.scrolling || state.near || state.hovered || Boolean(state.dragging);
    state.vertical.rail.classList.toggle("is-visible", visible);
    state.horizontal.rail.classList.toggle("is-visible", visible);
  }

  function updateNativeOverlay(state) {
    const { element } = state;
    if (!element.isConnected) {
      removeNativeOverlay(state);
      return;
    }
    const { vertical, horizontal } = scrollAxes(element);
    const rect = element.getBoundingClientRect();
    const left = Math.max(0, rect.left);
    const right = Math.min(window.innerWidth, rect.right);
    const top = Math.max(0, rect.top);
    const bottom = Math.min(window.innerHeight, rect.bottom);
    const verticalLength = Math.max(0, bottom - top - 4);
    const horizontalLength = Math.max(0, right - left - 4);

    state.vertical.rail.classList.toggle("is-enabled", vertical && verticalLength > 0);
    state.horizontal.rail.classList.toggle("is-enabled", horizontal && horizontalLength > 0);

    if (vertical && verticalLength > 0) {
      const thumbSize = Math.min(
        verticalLength,
        Math.max(MIN_THUMB_SIZE, verticalLength * element.clientHeight / element.scrollHeight),
      );
      const travel = Math.max(0, verticalLength - thumbSize);
      const maxScroll = Math.max(1, element.scrollHeight - element.clientHeight);
      state.vertical.rail.style.top = `${top + 2}px`;
      state.vertical.rail.style.left = `${Math.max(left, right - 7)}px`;
      state.vertical.rail.style.height = `${verticalLength}px`;
      state.vertical.thumb.style.height = `${thumbSize}px`;
      state.vertical.thumb.style.transform = `translateY(${travel * element.scrollTop / maxScroll}px)`;
    }

    if (horizontal && horizontalLength > 0) {
      const thumbSize = Math.min(
        horizontalLength,
        Math.max(MIN_THUMB_SIZE, horizontalLength * element.clientWidth / element.scrollWidth),
      );
      const travel = Math.max(0, horizontalLength - thumbSize);
      const maxScroll = Math.max(1, element.scrollWidth - element.clientWidth);
      state.horizontal.rail.style.top = `${Math.max(top, bottom - 7)}px`;
      state.horizontal.rail.style.left = `${left + 2}px`;
      state.horizontal.rail.style.width = `${horizontalLength}px`;
      state.horizontal.thumb.style.width = `${thumbSize}px`;
      state.horizontal.thumb.style.transform = `translateX(${travel * element.scrollLeft / maxScroll}px)`;
    }
  }

  function removeNativeOverlay(state) {
    resizeObserver.unobserve(state.element);
    window.clearTimeout(state.hideTimer);
    state.vertical.rail.remove();
    state.horizontal.rail.remove();
    nativeOverlays.delete(state.element);
  }

  function requestOverlayUpdate() {
    if (overlayFrame) return;
    overlayFrame = window.requestAnimationFrame(() => {
      overlayFrame = 0;
      for (const state of nativeOverlays.values()) updateNativeOverlay(state);
    });
  }

  document.addEventListener("scroll", onScroll, true);
  document.addEventListener("pointermove", onPointerMove, { passive: true });
  window.addEventListener("resize", requestOverlayUpdate);
  window.addEventListener("blur", clearNearOwners);
  document.documentElement.addEventListener("pointerleave", clearNearOwners);

  return () => {
    document.removeEventListener("scroll", onScroll, true);
    document.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("resize", requestOverlayUpdate);
    window.removeEventListener("blur", clearNearOwners);
    document.documentElement.removeEventListener("pointerleave", clearNearOwners);
    if (pointerFrame) window.cancelAnimationFrame(pointerFrame);
    if (overlayFrame) window.cancelAnimationFrame(overlayFrame);
    for (const timer of naiveHideTimers.values()) window.clearTimeout(timer);
    for (const owner of naiveHideTimers.keys()) owner.classList.remove(SCROLLING_CLASS);
    for (const state of [...nativeOverlays.values()]) removeNativeOverlay(state);
    resizeObserver.disconnect();
    clearNearOwners();
  };
}
