const SCROLLING_CLASS = "scrollbar-active";
const NEAR_CLASS = "scrollbar-near";
const HIDE_DELAY = 900;
const EDGE_DISTANCE = 12;

// 统一管理原生和 Naive UI 滚动容器的自动显隐状态。
export function installAutoHideScrollbars() {
  const hideTimers = new Map();
  let nearOwners = new Set();
  let pointerFrame = 0;
  let latestPointer = null;

  function ownerFor(element) {
    if (element.classList.contains("n-scrollbar-container")) {
      return element.closest(".n-scrollbar") || element;
    }
    return element;
  }

  function showWhileScrolling(element) {
    const owner = ownerFor(element);
    owner.classList.add(SCROLLING_CLASS);
    window.clearTimeout(hideTimers.get(owner));
    hideTimers.set(owner, window.setTimeout(() => {
      owner.classList.remove(SCROLLING_CLASS);
      hideTimers.delete(owner);
    }, HIDE_DELAY));
  }

  function onScroll(event) {
    if (event.target instanceof Element) showWhileScrolling(event.target);
  }

  function scrollAxes(element) {
    const style = window.getComputedStyle(element);
    const vertical = /(auto|scroll|overlay)/.test(style.overflowY)
      && element.scrollHeight > element.clientHeight + 1;
    const horizontal = /(auto|scroll|overlay)/.test(style.overflowX)
      && element.scrollWidth > element.clientWidth + 1;
    return { vertical, horizontal };
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
      if (isNearScrollbar(current, point)) owners.add(ownerFor(current));
      current = current.parentElement;
    }
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
      if (!nextOwners.has(owner)) owner.classList.remove(NEAR_CLASS);
    }
    for (const owner of nextOwners) owner.classList.add(NEAR_CLASS);
    nearOwners = nextOwners;
  }

  function onPointerMove(event) {
    latestPointer = { x: event.clientX, y: event.clientY };
    if (!pointerFrame) pointerFrame = window.requestAnimationFrame(updateNearOwners);
  }

  function clearNearOwners() {
    latestPointer = null;
    for (const owner of nearOwners) owner.classList.remove(NEAR_CLASS);
    nearOwners.clear();
  }

  document.addEventListener("scroll", onScroll, true);
  document.addEventListener("pointermove", onPointerMove, { passive: true });
  window.addEventListener("blur", clearNearOwners);
  document.documentElement.addEventListener("pointerleave", clearNearOwners);

  return () => {
    document.removeEventListener("scroll", onScroll, true);
    document.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("blur", clearNearOwners);
    document.documentElement.removeEventListener("pointerleave", clearNearOwners);
    if (pointerFrame) window.cancelAnimationFrame(pointerFrame);
    for (const timer of hideTimers.values()) window.clearTimeout(timer);
    for (const owner of hideTimers.keys()) owner.classList.remove(SCROLLING_CLASS);
    clearNearOwners();
  };
}
