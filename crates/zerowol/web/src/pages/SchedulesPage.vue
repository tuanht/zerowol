<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { f7 } from "framework7-vue";
import Icon from "../components/Icon.vue";
import ZonePicker from "../components/ZonePicker.vue";
import WeekStrip from "../components/WeekStrip.vue";
import DayAgenda from "../components/DayAgenda.vue";
import WeekAgenda from "../components/WeekAgenda.vue";
import { IOS } from "../lib/layout.js";
import { offlineMsg } from "../lib/api.js";
import { toast } from "../lib/ui.js";
import { addDays, fmtLong, weekDays, weekStart } from "../lib/dates.js";
import { fetchOccurrences, reloadSchedules, schedState, setTimezone, todayIn } from "../lib/schedules.js";
import { openOccurrenceActions } from "../lib/occurrences.js";

const ios = IOS;
const add = inject("addFor")["view-schedules"];
const VIEW_KEY = "zw-sched-view";
const VIEWS = { day: "Day", week: "Week" };
const readView = () => {
  try {
    return localStorage.getItem(VIEW_KEY) === "week" ? "week" : "day";
  } catch {
    return "day";
  }
};
const view = ref(readView());
watch(view, (v) => {
  try {
    localStorage.setItem(VIEW_KEY, v);
  } catch {}
});

// "Today" is the schedule timezone's date (the browser's until one is set).
const tz = computed(() => schedState.settings && schedState.settings.timezone);
const today = ref(todayIn());
const selected = ref(today.value);
const days = computed(() => weekDays(selected.value));
const occs = ref([]);
const loaded = ref(false);
const now = ref(Date.now() / 1000);
const dayOccs = computed(() => occs.value.filter((o) => o.date === selected.value));
const busy = ref(false);

let seq = 0;
async function load() {
  const n = ++seq;
  try {
    await reloadSchedules();
    today.value = todayIn(tz.value);
    if (!loaded.value) selected.value = today.value;
    const from = weekStart(selected.value);
    const list = tz.value ? await fetchOccurrences(from, addDays(from, 6)) : [];
    if (n !== seq) return; // a newer load (other week) is on its way
    occs.value = list;
    now.value = Date.now() / 1000;
    loaded.value = true;
  } catch {
    // Offline: the app-wide banner says so; keep what is shown.
  }
}
watch(() => weekStart(selected.value), () => loaded.value && load());
watch(() => schedState.rev, () => load());

const goToday = () => (selected.value = today.value);
const shiftWeek = (n) => (selected.value = addDays(selected.value, 7 * n));
function pickDay(d) {
  selected.value = d;
  view.value = "day";
}
const pick = (o, el) => openOccurrenceActions(o, el);

async function useZone(name) {
  if (!name || busy.value) return;
  busy.value = true;
  try {
    await setTimezone(name);
    await load();
  } catch (e) {
    toast(offlineMsg(e));
  }
  busy.value = false;
}

// Reload on showing the tab, and every 60 s while the tab and the document are visible.
const pageEl = ref(null);
let timer = null;
function sync() {
  const el = pageEl.value && pageEl.value.$el;
  const shown = !!el && !!el.closest(".tab-active") && document.visibilityState === "visible";
  if (shown && !timer) {
    load();
    timer = setInterval(load, 60e3);
  } else if (!shown && timer) {
    clearInterval(timer);
    timer = null;
  }
}
onMounted(() => {
  f7.on("tabShow tabHide", sync);
  document.addEventListener("visibilitychange", sync);
  sync();
});
onBeforeUnmount(() => {
  f7.off("tabShow tabHide", sync);
  document.removeEventListener("visibilitychange", sync);
  clearInterval(timer);
});
</script>

<template>
  <f7-page ref="pageEl" name="schedules">
    <f7-navbar title="Schedules" large>
      <template #left><f7-link @click="goToday">Today</f7-link></template>
      <f7-nav-right v-if="ios">
        <f7-link icon-only aria-label="Add Schedule" @click="add"><Icon name="plus" /></f7-link>
      </f7-nav-right>
    </f7-navbar>

    <template v-if="schedState.settings && !tz">
      <ZonePicker :busy="busy" @pick="useZone" />
    </template>
    <template v-else-if="loaded">
      <div class="block">
        <div class="segmented" :class="{ 'segmented-strong': ios }" role="radiogroup" aria-label="View">
          <button
            v-for="(label, v) in VIEWS"
            :key="v"
            class="button"
            :class="{ 'button-active': view === v }"
            type="button"
            role="radio"
            :aria-checked="view === v"
            @click="view = v"
          >
            {{ label }}
          </button>
          <span v-if="ios" class="segmented-highlight" />
        </div>
      </div>

      <WeekStrip :days="days" :selected="view === 'day' ? selected : ''" :today="today" :occurrences="occs" @select="pickDay" @week="shiftWeek" />

      <div v-if="!occs.length" class="block text-align-center">
        <p>No schedules this week</p>
        <button class="button button-fill button-round" type="button" @click="add">Add schedule</button>
      </div>
      <template v-else-if="view === 'day'">
        <div class="block-title">{{ fmtLong(selected) }}<template v-if="selected === today"> · today</template></div>
        <DayAgenda :occurrences="dayOccs" :now="now" @pick="pick" />
      </template>
      <WeekAgenda v-else :days="days" :today="today" :occurrences="occs" :now="now" @pick="pick" />
    </template>
  </f7-page>
</template>
