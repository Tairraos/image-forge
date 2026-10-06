<template>
  <img :src="logo" alt="Image Forge" />
</template>

<script setup>
import { computed } from 'vue';
import darkLogo from '../../assets/title-dark.png';
import lightLogo from '../../assets/title-light.png';

const props = defineProps({
  theme: { type: String, default: 'light' },
});

const logo = computed(() => (props.theme === 'dark' ? darkLogo : lightLogo));
</script>
