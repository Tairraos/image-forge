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
  "n-select": { template: "<select />" },
  "n-dropdown": { template: "<div><slot /></div>" },
};
