import { enableAutoUnmount } from '@vue/test-utils';
import { afterEach } from 'vitest';

enableAutoUnmount(afterEach);

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
