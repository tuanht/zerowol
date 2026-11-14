<script setup>
import { computed } from "vue";
import Icon from "./Icon.vue";
import { fmtLong, fmtRange } from "../lib/dates.js";

const props = defineProps({
  days: { type: Array, required: true }, // 7 dates, Monday first
  selected: { type: String, default: "" },
  today: { type: String, default: "" },
  occurrences: { type: Array, default: () => [] },
});
const emit = defineEmits(["select", "week"]);
const LETTERS = ["M", "T", "W", "T", "F", "S", "S"];
const ORDER = ["wake", "reboot", "poweroff"];

// One dot per action colour per day.
const dots = computed(() => {
  const by = {};
  for (const o of props.occurrences) (by[o.date] ||= new Set()).add(o.action);
  return Object.fromEntries(Object.entries(by).map(([d, s]) => [d, ORDER.filter((a) => s.has(a))]));
});
const title = computed(() => fmtRange(props.days[0], props.days[6]));

// Horizontal swipe on the strip changes the week (left = next).
let x0 = null;
const down = (e) => (x0 = e.clientX);
function up(e) {
  if (x0 == null) return;
  const dx = e.clientX - x0;
  x0 = null;
  if (Math.abs(dx) > 40) emit("week", dx < 0 ? 1 : -1);
}
</script>

<template>
  <div class="block-title">{{ title }}</div>
  <div class="block zw-strip" @pointerdown="down" @pointerup="up" @pointercancel="x0 = null">
    <a class="link icon-only" href="#" aria-label="Previous week" @click.prevent="emit('week', -1)"><Icon name="prev" /></a>
    <button
      v-for="(d, i) in days"
      :key="d"
      class="zw-day"
      :class="{ sel: d === selected, today: d === today }"
      type="button"
      :aria-label="fmtLong(d)"
      :aria-pressed="d === selected"
      @click="emit('select', d)"
    >
      <span class="zw-wd">{{ LETTERS[i] }}</span>
      <span class="zw-num">{{ +d.slice(8) }}</span>
      <span class="zw-dots"><span v-for="a in dots[d] || []" :key="a" class="dot" :class="a" /></span>
    </button>
    <a class="link icon-only" href="#" aria-label="Next week" @click.prevent="emit('week', 1)"><Icon name="next" /></a>
  </div>
</template>
