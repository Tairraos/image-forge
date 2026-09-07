// 使用原生滚动条，滚动结束后淡出；悬停和键盘聚焦状态由 CSS 处理。
export function installAutoHideScrollbars() {
  const timers = new Map();
  function onScroll(event) {
    const element = event.target;
    if (!(element instanceof HTMLElement)) return;
    element.classList.add('scrollbar-active');
    window.clearTimeout(timers.get(element));
    timers.set(
      element,
      window.setTimeout(() => {
        element.classList.remove('scrollbar-active');
        timers.delete(element);
      }, 900)
    );
  }
  document.addEventListener('scroll', onScroll, true);
  return () => {
    document.removeEventListener('scroll', onScroll, true);
    for (const [element, timer] of timers) {
      window.clearTimeout(timer);
      element.classList.remove('scrollbar-active');
    }
    timers.clear();
  };
}
