<template>
  <main class="app app--agent">
    <div class="app-shell-workspace">
      <slot />
    </div>

    <slot name="footer" />
    <slot name="dialogs" />
  </main>
</template>
