<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { f7 } from "framework7-vue";
import Icon from "../components/Icon.vue";
import ZonePicker from "../components/ZonePicker.vue";
import WeekStrip from "../components/WeekStrip.vue";
import DayAgenda from "../components/DayAgenda.vue";
import WeekAgenda from "../components/WeekAgenda.vue";
import WeekGrid from "../components/WeekGrid.vue";
import MonthGrid from "../components/MonthGrid.vue";
import { IOS, wide } from "../lib/layout.js";
import { offlineMsg } from "../lib/api.js";
import { toast } from "../lib/ui.js";
import { state as machines } from "../lib/machines.js";
import { addDays, addMonths, fmtLong, fmtRange, monthGrid, weekDays } from "../lib/dates.js";
import { fetchOccurrences, reloadSchedules, schedState, setTimezone, todayIn } from "../lib/schedules.js";
import { openDayPopover, openOccurrenceActions } from "../lib/occurrences.js";

const ios = IOS;
const add = inject("addFor")["view-schedules"];
// Phone: Day (default) | Week. Wide (>= 768 px): Week (default) | Month. Each set remembers its own choice,
// and rotating / resizing across the breakpoint swaps the set while keeping the selected date.
const SETS = {
  phone: { key: "zw-sched-view", views: { day: "Day", week: "Week" } },
  wide: { key: "zw-sched-view-wide", views: { week: "Week", month: "Month" } },
};
function viewRef({ key, views }) {
  let v = null;
  try {
    v = localStorage.getItem(key);
  } catch {}
  const r = ref(Object.hasOwn(views, v) ? v : Object.keys(views)[0]);
  watch(r, (x) => {
    try {
      localStorage.setItem(key, x);
    } catch {}
  });
  return r;
}
const views = { phone: viewRef(SETS.phone), wide: viewRef(SETS.wide) };
const set = computed(() => (wide.value ? "wide" : "phone"));
const VIEWS = computed(() => SETS[set.value].views);
const view = computed({
  get: () => views[set.value].value,
  set: (v) => (views[set.value].value = v),
});
const isMonth = computed(() => view.value === "month");
const span = computed(() => (isMonth.value ? "month" : "week"));

// "Today" is the schedule timezone's date (the browser's until one is set).
const tz = computed(() => schedState.settings && schedState.settings.timezone);
const today = ref(todayIn());
const selected = ref(today.value);
const days = computed(() => weekDays(selected.value));
// The dates on screen: the week, or the month grid's 42 days.
const range = computed(() => {
  if (isMonth.value) {
    const g = monthGrid(selected.value);
    return [g[0], g[41]];
  }
  return [days.value[0], days.value[6]];
});
const title = computed(() => (isMonth.value ? fmtRange(selected.value, selected.value) : fmtRange(days.value[0], days.value[6])));
const occs = ref([]);
const loaded = ref(false);
const now = ref(Date.now() / 1000);
const dayOccs = computed(() => occs.value.filter((o) => o.date === selected.value));
const busy = ref(false);

let seq = 0;
let held = ""; // the range `occs` belongs to
const failed = ref(false); // this range's occurrences could not be loaded
async function load() {
  const n = ++seq;
  let key = range.value.join();
  try {
    await reloadSchedules();
    today.value = todayIn(tz.value);
    if (!loaded.value) selected.value = today.value;
    const [from, to] = range.value;
    key = from + "," + to;
    const list = tz.value ? await fetchOccurrences(from, to) : [];
    if (n !== seq) return; // a newer load (other range) is on its way
    occs.value = list;
    held = key;
    failed.value = false;
    now.value = Date.now() / 1000;
    loaded.value = true;
  } catch (e) {
    if (n !== seq) return;
    // Never show another range's occurrences under this range's dates.
    if (held !== key) {
      occs.value = [];
      held = "";
      failed.value = true;
      loaded.value = true; // show the strip and the failure line, not a blank page
    }
    // Offline is already announced by the app-wide banner.
    if (!machines.offline) toast(offlineMsg(e));
  }
}
watch(() => range.value.join(), () => loaded.value && load());
watch(() => schedState.rev, () => load());

const goToday = () => (selected.value = today.value);
// Previous / next: by month in Month view, by week otherwise.
const shift = (n) => (selected.value = isMonth.value ? addMonths(selected.value, n) : addDays(selected.value, 7 * n));
function pickDay(d) {
  selected.value = d;
  view.value = "day";
}
const pick = (o, el) => openOccurrenceActions(o, el);
const pickDate = (d, el) => openDayPopover(fmtLong(d), occs.value.filter((o) => o.date === d), el, now.value);

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
      <!-- Wide: our wrapper keeps the bar and grids out of the 640 px cap app.css puts on the page's direct blocks. -->
      <div v-if="wide">
        <div class="block">
          <div class="zw-bar">
            <a class="link icon-only" href="#" :aria-label="'Previous ' + span" @click.prevent="shift(-1)"><Icon name="prev" /></a>
            <a class="link icon-only" href="#" :aria-label="'Next ' + span" @click.prevent="shift(1)"><Icon name="next" /></a>
            <span class="zw-bar-title">{{ title }}</span>
            <div class="zw-bar-seg">
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
          </div>
          <template v-if="occs.length && !failed">
            <MonthGrid v-if="isMonth" :month="selected" :occurrences="occs" :today="today" :now="now" @pick="pickDate" />
            <WeekGrid v-else :days="days" :occurrences="occs" :today="today" :now="now" @pick="pick" />
          </template>
        </div>
      </div>
      <template v-else>
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

        <WeekStrip :days="days" :selected="view === 'day' ? selected : ''" :today="today" :occurrences="occs" @select="pickDay" @week="shift" />
      </template>

      <div v-if="failed" class="block text-align-center">
        <p>Couldn't load this {{ span }}'s schedules.</p>
      </div>
      <div v-else-if="!occs.length" class="block text-align-center">
        <p>No schedules this {{ span }}</p>
        <button class="button button-fill button-round" type="button" @click="add">Add schedule</button>
      </div>
      <template v-else-if="!wide && view === 'day'">
        <div class="block-title">{{ fmtLong(selected) }}<template v-if="selected === today"> · today</template></div>
        <DayAgenda :occurrences="dayOccs" :now="now" @pick="pick" />
      </template>
      <WeekAgenda v-else-if="!wide" :days="days" :today="today" :occurrences="occs" :now="now" @pick="pick" />
    </template>
  </f7-page>
</template>
