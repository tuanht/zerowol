<script setup>
import { IOS } from "../lib/layout.js";

defineProps({
  views: { type: Object, required: true }, // value -> label
  modelValue: { type: String, required: true },
});
const emit = defineEmits(["update:modelValue"]);
const ios = IOS;
</script>

<template>
  <div class="segmented" :class="{ 'segmented-strong': ios }" role="radiogroup" aria-label="View">
    <button
      v-for="(label, v) in views"
      :key="v"
      class="button"
      :class="{ 'button-active': modelValue === v }"
      type="button"
      role="radio"
      :aria-checked="modelValue === v"
      @click="emit('update:modelValue', v)"
    >
      {{ label }}
    </button>
    <span v-if="ios" class="segmented-highlight" />
  </div>
</template>
