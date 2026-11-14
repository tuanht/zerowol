<script setup>
import { computed } from "vue";
import DayAgenda from "./DayAgenda.vue";
import { fmtDay } from "../lib/dates.js";

const props = defineProps({
  days: { type: Array, required: true },
  today: { type: String, default: "" },
  occurrences: { type: Array, required: true },
  now: { type: Number, required: true },
});
const emit = defineEmits(["pick"]);
// Days that have occurrences, in order.
const groups = computed(() =>
  props.days.map((d) => ({ d, items: props.occurrences.filter((o) => o.date === d) })).filter((g) => g.items.length),
);
</script>

<template>
  <template v-for="g in groups" :key="g.d">
    <div class="block-title">{{ fmtDay(g.d) }}<template v-if="g.d === today"> · today</template></div>
    <DayAgenda :occurrences="g.items" :now="now" @pick="(o, el) => emit('pick', o, el)" />
  </template>
</template>
