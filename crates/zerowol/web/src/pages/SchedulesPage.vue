<script setup>
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { f7 } from "framework7-vue";
import Icon from "../components/Icon.vue";
import ZonePicker from "../components/ZonePicker.vue";
import WeekStrip from "../components/WeekStrip.vue";
import DayAgenda from "../components/DayAgenda.vue";
import WeekAgenda from "../components/WeekAgenda.vue";
import Segmented from "../components/Segmented.vue";
import WeekGrid from "../components/WeekGrid.vue";
import MonthGrid from "../components/MonthGrid.vue";
import { IOS, wide } from "../lib/layout.js";
import { offlineMsg } from "../lib/api.js";
import { toast } from "../lib/ui.js";
import { state as machines } from "../lib/machines.js";
import { addDays, addMonths, fmtLong, fmtRange, monthGrid, weekDays } from "../lib/dates.js";
import { clockIn, fetchOccurrences, openScheduleForm, reloadSchedules, schedState, setTimezone, todayIn } from "../lib/schedules.js";
import { openOccurrenceActions } from "../lib/occurrences.js";

const props = defineProps({ f7router: { type: Object, required: true } });
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
const held = ref(""); // the range `occs` belongs to
// Only show occurrences under the range they were loaded for (e.g. not a week's worth in a fresh Month view).
const ready = computed(() => held.value === range.value.join());
const failed = ref(false); // this range's occurrences could not be loaded
let toasted = false; // the current failure streak has been announced (the 60 s reload doesn't repeat it)
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
    held.value = key;
    failed.value = false;
    now.value = Date.now() / 1000;
    loaded.value = true;
    toasted = false;
  } catch (e) {
    if (n !== seq) return;
    // Never show another range's occurrences under this range's dates.
    if (held.value !== key) {
      occs.value = [];
      held.value = "";
      failed.value = true;
      loaded.value = true; // show the strip and the failure line, not a blank page
    }
    // Offline is already announced by the app-wide banner.
    if (!machines.offline && !toasted) {
      toast(offlineMsg(e));
      toasted = true;
    }
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
// Long press / double-click on a free slot (wide grids): the New Schedule form for a one-off at that date and time.
// Saving bumps schedState.rev, which reloads this page as for the + button.
const create = (prefill) => openScheduleForm({ prefill: { kind: "once", ...prefill } });
// The week grid's current-time line: now in the schedule timezone, ticking every 30 s while the tab is shown.
const clock = ref(clockIn(tz.value));
const tick = () => (clock.value = clockIn(tz.value));
watch(tz, tick);

// Month view: tapping a day opens its agenda (our DayAgenda) in an F7 popover anchored to the cell.
// The popover element lives in this page's template; F7 moves it out while open and puts it back on close.
const popEl = ref(null);
const popDate = ref("");
const popOccs = computed(() => occs.value.filter((o) => o.date === popDate.value));
let pop = null;
let popCell = null;
let popPicked = null;
async function pickDate(d, el) {
  popDate.value = d;
  popCell = el;
  popPicked = null;
  pop ||= f7.popover.create({
    el: popEl.value,
    on: {
      // Tapping an item closes the popover first, then opens that occurrence's actions at the same cell.
      closed: () => popPicked && openOccurrenceActions(popPicked, popCell),
    },
  });
  // Open once the popover shows the new day, so F7 sizes and places it for that content.
  await nextTick();
  pop.open(el);
}
function popPick(o) {
  popPicked = o;
  pop.close();
}

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

// "Choose another…" opens the timezone page in this tab; picking there saves and reloads like useZone.
const moreZones = () =>
  props.f7router.navigate("/timezone/", {
    props: {
      pick: async (name) => {
        await setTimezone(name);
        await load();
      },
    },
  });

// Reload on showing the tab, and every 60 s while the tab and the document are visible.
const pageEl = ref(null);
let timer = null;
let ticker = null;
function sync() {
  const el = pageEl.value && pageEl.value.$el;
  const shown = !!el && !!el.closest(".tab-active") && document.visibilityState === "visible";
  if (shown && !timer) {
    load();
    timer = setInterval(load, 60e3);
    tick();
    ticker = setInterval(tick, 30e3);
  } else if (!shown && timer) {
    clearInterval(timer);
    clearInterval(ticker);
    timer = null;
  }
}
onMounted(() => {
  f7.on("tabShow tabHide", sync);
  document.addEventListener("visibilitychange", sync);
  sync();
});
onBeforeUnmount(() => {
  if (pop) pop.destroy();
  f7.off("tabShow tabHide", sync);
  document.removeEventListener("visibilitychange", sync);
  clearInterval(timer);
  clearInterval(ticker);
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
      <ZonePicker :busy="busy" @pick="useZone" @more="moreZones" />
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
              <Segmented v-model="view" :options="VIEWS" label="View" />
            </div>
          </div>
          <!-- drawn even without occurrences: the free slots are where a long press adds one -->
          <template v-if="ready && !failed">
            <MonthGrid v-if="isMonth" :month="selected" :occurrences="occs" :today="today" :now="now" :zone="tz || ''" @pick="pickDate" @create="create" />
            <WeekGrid v-else :days="days" :occurrences="occs" :today="today" :now="now" :clock="clock" :zone="tz || ''" @pick="pick" @create="create" />
            <div class="block-footer">Long-press or double-click {{ isMonth ? "a day" : "a time" }} to add a schedule.</div>
          </template>
        </div>
      </div>
      <template v-else>
        <div class="block">
          <Segmented v-model="view" :options="VIEWS" label="View" />
        </div>

        <WeekStrip :days="days" :selected="view === 'day' ? selected : ''" :today="today" :occurrences="ready ? occs : []" :now="now" @select="pickDay" @week="shift" />
      </template>

      <div v-if="failed" class="block text-align-center">
        <p>Couldn't load this {{ span }}'s schedules.</p>
      </div>
      <template v-else-if="!ready" />
      <div v-else-if="!occs.length && !wide" class="block text-align-center">
        <p>No schedules this {{ span }}</p>
        <button class="button button-fill button-round" type="button" @click="add">Add schedule</button>
      </div>
      <template v-else-if="!wide && view === 'day'">
        <div class="block-title">{{ fmtLong(selected) }}<template v-if="selected === today"> · today</template></div>
        <DayAgenda :occurrences="dayOccs" :now="now" @pick="pick" />
      </template>
      <WeekAgenda v-else-if="!wide" :days="days" :today="today" :occurrences="occs" :now="now" @pick="pick" />
    </template>

    <!-- Month view's day popover (F7 Popover; opened and anchored from pickDate). -->
    <div ref="popEl" class="popover zw-day-pop">
      <div class="popover-inner">
        <div class="block-title">{{ popDate && fmtLong(popDate) }}</div>
        <DayAgenda :occurrences="popOccs" :now="now" @pick="popPick" />
      </div>
    </div>
  </f7-page>
</template>
