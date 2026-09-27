import { enableAutoUnmount } from '@vue/test-utils';
import { afterEach } from 'vitest';

enableAutoUnmount(afterEach);

// 以下均为 jsdom（浏览器环境）专用 polyfill；node 环境的测试文件（如 api/blob-server.spec.js）没有这些全局对象。
if (typeof HTMLDialogElement !== 'undefined') {
  // jsdom does not implement the browser's native dialog top layer.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    if (!this.open) return;
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };

  window.matchMedia = (media) => ({
    media,
    matches: false,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  });
}
