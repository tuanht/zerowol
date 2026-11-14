<script setup>
import { computed, nextTick, onMounted, ref } from "vue";
import { ACTION } from "../lib/words.js";
import { fmtDay } from "../lib/dates.js";
import { isOff } from "../lib/occurrences.js";
import { clockIn } from "../lib/schedules.js";
import { longPress, slotTime } from "../lib/longpress.js";

const props = defineProps({
  days: { type: Array, required: true }, // 7 dates, Monday first
  occurrences: { type: Array, required: true },
  today: { type: String, default: "" },
  now: { type: Number, default: 0 }, // unix seconds
  clock: { type: Object, default: null }, // now in the schedule timezone: { date, min }
  zone: { type: String, default: "" }, // the schedule timezone
});
const emit = defineEmits(["pick", "create"]);

const pad = (n) => String(n).padStart(2, "0");
// The current-time line: shown when this week holds today, in the current hour's row at its minute.
const nowHour = computed(() => (props.clock && props.days.includes(props.clock.date) ? Math.floor(props.clock.min / 60) : -1));
const nowTop = computed(() => (props.clock ? ((props.clock.min % 60) / 60) * 100 + "%" : "0"));
const nowLabel = computed(() => props.clock && pad(nowHour.value) + ":" + pad(props.clock.min % 60));

// All 24 hours; the current hour's label is the current time, and the next hour's label gives way when they'd touch.
const rows = computed(() => {
  const by = {};
  for (const o of props.occurrences) ((by[+o.time.slice(0, 2)] ||= {})[o.date] ||= []).push(o);
  const near = nowHour.value >= 0 && props.clock.min % 60 >= 50 ? nowHour.value + 1 : -1;
  return Array.from({ length: 24 }, (_, h) => ({
    h,
    label: pad(h) + ":00",
    now: h === nowHour.value,
    hide: h === near,
    cells: props.days.map((d) => (by[h] && by[h][d]) || []),
  }));
});
const text = (o) => o.machine_name + " · " + ACTION[o.action];

// Long press / double-click on a cell's empty part: a one-off rule at that slot, unless it's over.
// A long press on a chip opens its actions, as a tap does (touch browsers send no click after a long press).
const lp = longPress({
  find: (t) => t.closest(".zw-chip") || t.closest(".zw-wcell"),
  resolve: (el) => {
    if (el.classList.contains("zw-chip")) {
      const date = el.closest(".zw-wcell").dataset.date;
      const o = props.occurrences.find((x) => x.schedule_id === el.dataset.id && x.date === date);
      return o && (() => emit("pick", o, el));
    }
    const { date, hour } = el.dataset;
    const time = slotTime(date, +hour, clockIn(props.zone));
    return time && (() => emit("create", { date, time }));
  },
});

// On opening a week: scroll the current hour (or the week's first occurrence, else 07:00) near the top,
// with about a row and a half above it under the sticky day header. Re-renders (the periodic reload) keep the scroll.
const root = ref(null);
function scrollToStart() {
  const g = root.value;
  const pc = g && g.closest(".page-content");
  if (!pc) return;
  const first = props.occurrences.map((o) => o.time).sort()[0];
  const h = nowHour.value >= 0 ? nowHour.value : first ? +first.slice(0, 2) : 7;
  if (h < 2) return void (pc.scrollTop = 0); // the first hours: the page's top, with the bar above the grid
  const head = g.querySelector(".zw-whead");
  const row = g.querySelectorAll(".zw-whour")[h - 1].getBoundingClientRect();
  // Where the header's bottom sits once stuck (sticky offsets count from the page-content's padding edge).
  const under = pc.getBoundingClientRect().top + parseFloat(getComputedStyle(pc).paddingTop) + parseFloat(getComputedStyle(head).top) + head.offsetHeight;
  pc.scrollTop += row.top - under - row.height / 2;
}
onMounted(() => nextTick(() => requestAnimationFrame(scrollToStart)));
</script>

<template>
  <div
    ref="root"
    class="zw-wgrid"
    role="table"
    aria-label="Week"
    @pointerdown="lp.pointerdown"
    @pointermove="lp.pointermove"
    @pointerup="lp.end"
    @pointercancel="lp.end"
    @pointerleave="lp.end"
    @dblclick="(e) => e.target.closest('.zw-chip') || lp.dblclick(e)"
    @contextmenu="lp.contextmenu"
  >
    <div class="zw-wcorner" />
    <div v-for="d in days" :key="d" class="zw-whead" :class="{ today: d === today }" role="columnheader">
      {{ fmtDay(d) }}
    </div>
    <template v-for="r in rows" :key="r.h">
      <div class="zw-whour" :class="{ now: r.now, near: r.hide }" role="rowheader">
        <span v-if="r.now" class="zw-wnow-t" :style="{ top: nowTop }">{{ nowLabel }}</span>
        <template v-else>{{ r.label }}</template>
      </div>
      <div v-for="(list, i) in r.cells" :key="i" class="zw-wcell" :class="{ today: days[i] === today }" role="cell" :data-date="days[i]" :data-hour="r.h">
        <button
          v-for="o in list"
          :key="o.schedule_id"
          class="zw-chip"
          :data-id="o.schedule_id"
          :class="isOff(o, now) ? 'off' : o.action"
          type="button"
          :title="o.time + ' ' + text(o) + (o.skipped ? ' (skipped)' : '')"
          @click="emit('pick', o, $event.currentTarget)"
        >
          <component :is="o.skipped ? 's' : 'span'">{{ o.time }} {{ text(o) }}</component>
        </button>
        <div v-if="r.now" class="zw-wnow" :class="{ today: days[i] === clock.date }" :style="{ top: nowTop }" aria-hidden="true" />
      </div>
    </template>
  </div>
</template>
