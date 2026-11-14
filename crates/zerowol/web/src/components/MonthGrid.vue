<script setup>
import { computed } from "vue";
import { ACTION } from "../lib/words.js";
import { fmtLong, monthGrid } from "../lib/dates.js";
import { isOff } from "../lib/occurrences.js";

const props = defineProps({
  month: { type: String, required: true }, // any date in the month
  occurrences: { type: Array, required: true },
  today: { type: String, default: "" },
  now: { type: Number, default: 0 }, // unix seconds
});
const emit = defineEmits(["pick"]);
const HEAD = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const ORDER = ["wake", "reboot", "poweroff", "off"];

// Per day: one dot per colour for repeating rules, a chip per once rule.
const cells = computed(() => {
  const by = {};
  for (const o of props.occurrences) (by[o.date] ||= []).push(o);
  const ym = props.month.slice(0, 7);
  return monthGrid(props.month).map((d) => {
    const list = by[d] || [];
    const colours = new Set(list.filter((o) => o.repeat_kind !== "once").map((o) => (isOff(o, props.now) ? "off" : o.action)));
    const chips = list.filter((o) => o.repeat_kind === "once");
    return { d, out: d.slice(0, 7) !== ym, dots: ORDER.filter((c) => colours.has(c)), chips: chips.slice(0, 2), more: chips.length - 2 };
  });
});
</script>

<template>
  <div class="zw-mgrid">
    <div v-for="h in HEAD" :key="h" class="zw-mhead">{{ h }}</div>
    <button
      v-for="c in cells"
      :key="c.d"
      class="zw-mcell"
      :class="{ out: c.out, today: c.d === today }"
      type="button"
      :aria-label="fmtLong(c.d)"
      @click="emit('pick', c.d, $event.currentTarget)"
    >
      <span class="zw-mtop">
        <span class="zw-mnum">{{ +c.d.slice(8) }}</span>
        <span class="zw-dots"><span v-for="a in c.dots" :key="a" class="dot" :class="a" /></span>
        <span v-if="c.more > 0" class="zw-more">+{{ c.more }}</span>
      </span>
      <span v-for="o in c.chips" :key="o.schedule_id" class="zw-chip" :class="isOff(o, now) ? 'off' : o.action">
        <component :is="o.skipped ? 's' : 'span'">{{ o.time }} {{ o.machine_name }} {{ ACTION[o.action] }}</component>
      </span>
    </button>
  </div>
</template>
