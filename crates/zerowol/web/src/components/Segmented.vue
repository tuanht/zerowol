<script setup>
import { IOS } from "../lib/layout.js";

// Framework7's segmented control in each platform's native form, built only from F7 classes:
// iOS: the capsule "strong" control with a sliding highlight (segmented-strong + segmented-round);
// Material: the M3 outlined segmented button, selected segment filled with the secondary container (button-tonal).
defineProps({
  options: { type: Object, required: true }, // value -> label
  modelValue: { type: String, required: true },
  label: { type: String, required: true },
  disabled: { type: Array, default: () => [] }, // values that can't be picked
  dots: { type: Boolean, default: false }, // action colour dot before each label
});
const emit = defineEmits(["update:modelValue"]);
const ios = IOS;
</script>

<template>
  <div class="segmented segmented-round" :class="{ 'segmented-strong': ios }" role="radiogroup" :aria-label="label">
    <button
      v-for="(text, v) in options"
      :key="v"
      class="button button-round"
      :class="{
        'button-outline': !ios,
        'button-active': ios && modelValue === v,
        'button-tonal': !ios && modelValue === v,
        disabled: disabled.includes(v),
      }"
      type="button"
      role="radio"
      :aria-checked="modelValue === v"
      :disabled="disabled.includes(v)"
      @click="emit('update:modelValue', v)"
    >
      <template v-if="dots"><span class="dot" :class="v" /><span>{{ text }}</span></template>
      <template v-else>{{ text }}</template>
    </button>
    <span v-if="ios" class="segmented-highlight" />
  </div>
</template>
