<script setup>
import { computed } from "vue";
import { ACTION } from "../lib/words.js";
import { fmtDay } from "../lib/dates.js";
import { isOff } from "../lib/occurrences.js";

const props = defineProps({
  days: { type: Array, required: true }, // 7 dates, Monday first
  occurrences: { type: Array, required: true },
  today: { type: String, default: "" },
  now: { type: Number, default: 0 }, // unix seconds
});
const emit = defineEmits(["pick"]);

// Rows: the hours that hold occurrences, with one "…" spacer for each run of empty hours between them.
const rows = computed(() => {
  const by = {};
  for (const o of props.occurrences) ((by[o.time.slice(0, 2)] ||= {})[o.date] ||= []).push(o);
  const out = [];
  let prev = null;
  for (const h of Object.keys(by).sort()) {
    if (prev != null && +h - prev > 1) out.push({ gap: true, key: "gap" + h });
    out.push({ key: h, label: h + ":00", cells: props.days.map((d) => by[h][d] || []) });
    prev = +h;
  }
  return out;
});
const text = (o) => o.machine_name + " · " + ACTION[o.action];
</script>

<template>
  <div class="zw-wgrid" role="table" aria-label="Week">
    <div class="zw-wcorner" />
    <div v-for="d in days" :key="d" class="zw-whead" :class="{ today: d === today }" role="columnheader">
      {{ fmtDay(d) }}
    </div>
    <template v-for="r in rows" :key="r.key">
      <div v-if="r.gap" class="zw-wgap" aria-hidden="true">…</div>
      <template v-else>
        <div class="zw-whour" role="rowheader">{{ r.label }}</div>
        <div v-for="(list, i) in r.cells" :key="i" class="zw-wcell" :class="{ today: days[i] === today }" role="cell">
          <button
            v-for="o in list"
            :key="o.schedule_id"
            class="zw-chip"
            :class="isOff(o, now) ? 'off' : o.action"
            type="button"
            :title="o.time + ' ' + text(o) + (o.skipped ? ' (skipped)' : '')"
            @click="emit('pick', o, $event.currentTarget)"
          >
            <component :is="o.skipped ? 's' : 'span'">{{ o.time }} {{ text(o) }}</component>
          </button>
        </div>
      </template>
    </template>
  </div>
</template>
