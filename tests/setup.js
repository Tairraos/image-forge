import { config } from "@vue/test-utils";

config.global.stubs = {
  "n-button": {
    emits: ["click"],
    template: '<button v-bind="$attrs" @click="$emit(\'click\', $event)"><slot name="icon" /><slot /></button>',
  },
  "n-button-group": { template: "<div><slot /></div>" },
  "n-checkbox": {
    props: ["checked"],
    emits: ["update:checked"],
    template: '<label><input type="checkbox" :checked="checked" @change="$emit(\'update:checked\', $event.target.checked)" /><slot /></label>',
  },
  "n-input": {
    props: ["value"],
    emits: ["update:value"],
    template: '<textarea :value="value" @input="$emit(\'update:value\', $event.target.value)" @keydown="$emit(\'keydown\', $event)" @paste="$emit(\'paste\', $event)" />',
  },
  "n-select": {
    props: ["value", "options"],
    emits: ["update:value"],
    template: '<select :value="value" @change="$emit(\'update:value\', $event.target.value)"><option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option></select>',
  },
  "n-dropdown": { template: "<div><slot /></div>" },
};
