<script setup>
import { computed, onBeforeUnmount } from "vue";
import { ACTION } from "../lib/words.js";
import { fmtLong, monthGrid } from "../lib/dates.js";
import { dotClass, isOff } from "../lib/occurrences.js";
import { clockIn } from "../lib/schedules.js";
import { dayTime, longPress } from "../lib/longpress.js";

const props = defineProps({
  month: { type: String, required: true }, // any date in the month
  occurrences: { type: Array, required: true },
  today: { type: String, default: "" },
  now: { type: Number, default: 0 }, // unix seconds
  zone: { type: String, default: "" }, // the schedule timezone
});
const emit = defineEmits(["pick", "create"]);
const HEAD = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const ORDER = ["wake", "reboot", "poweroff", "off"];

// Per day: one dot per colour for repeating rules, a chip per once rule.
const cells = computed(() => {
  const by = {};
  for (const o of props.occurrences) (by[o.date] ||= []).push(o);
  const ym = props.month.slice(0, 7);
  return monthGrid(props.month).map((d) => {
    const list = by[d] || [];
    const colours = new Set(list.filter((o) => o.repeat_kind !== "once").map((o) => dotClass(o, props.now)));
    const chips = list.filter((o) => o.repeat_kind === "once");
    return { d, out: d.slice(0, 7) !== ym, dots: ORDER.filter((c) => colours.has(c)), chips: chips.slice(0, 2), more: chips.length - 2 };
  });
});

// Long press / double-click on a day: a one-off rule that day at 08:00 (today: the next quarter hour once 08:00 is over).
const lp = longPress({
  find: (t) => t.closest(".zw-mcell"),
  resolve: (el) => {
    const date = el.dataset.date;
    const time = dayTime(date, clockIn(props.zone));
    return time && (() => emit("create", { date, time }));
  },
});
// A mouse click waits out the double-click interval before opening the day (the open popover would take the
// second click); touch and keyboard open it at once.
let mouse = false;
let wait = 0;
function down(e) {
  mouse = e.pointerType === "mouse";
  lp.pointerdown(e);
}
function tap(d, e) {
  clearTimeout(wait);
  const el = e.currentTarget;
  if (e.detail > 1) return; // the second click of a double-click
  if (mouse && e.detail === 1) wait = setTimeout(() => emit("pick", d, el), 300);
  else emit("pick", d, el);
}
onBeforeUnmount(() => clearTimeout(wait));
</script>

<template>
  <div
    class="zw-mgrid"
    @pointerdown="down"
    @pointermove="lp.pointermove"
    @pointerup="lp.end"
    @pointercancel="lp.end"
    @pointerleave="lp.end"
    @dblclick="lp.dblclick"
    @contextmenu="lp.contextmenu"
  >
    <div v-for="h in HEAD" :key="h" class="zw-mhead">{{ h }}</div>
    <button
      v-for="c in cells"
      :key="c.d"
      class="zw-mcell"
      :class="{ out: c.out, today: c.d === today }"
      type="button"
      :aria-label="fmtLong(c.d)"
      :data-date="c.d"
      @click="tap(c.d, $event)"
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
