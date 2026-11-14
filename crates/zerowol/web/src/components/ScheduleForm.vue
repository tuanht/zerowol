<script setup>
import { computed, nextTick, reactive, ref, watch } from "vue";
import { f7 } from "framework7-vue";
import Icon from "./Icon.vue";
import ZonePicker from "./ZonePicker.vue";
import { IOS, wide } from "../lib/layout.js";
import { offlineMsg } from "../lib/api.js";
import { confirmDialog } from "../lib/ui.js";
import { refresh, state as machines } from "../lib/machines.js";
import { ACTION, DAYS, DAY_LABEL, REPEAT } from "../lib/words.js";
import {
  clashes, closeScheduleForm, createSchedule, deleteSchedule, formState, reloadSchedules,
  schedState, setTimezone, todayIn, updateSchedule, weekdayOf,
} from "../lib/schedules.js";

const ios = IOS;
const opened = ref(false);
const step = ref("form"); // "tz" (timezone confirm, only while none is set) or "form"
const err = ref("");
const busy = ref(false);
const draft = reactive({ machine_id: "", action: "wake", time: "08:00", kind: "daily", date: "", days: [], enabled: true });
let result = false;
let shownSeq = 0; // the open (formState.seq) the popup is showing

const edit = computed(() => formState.schedule);
// Material on a wide screen is an M3 dialog: title on top, Cancel / Save at the bottom, no navbar.
const dialog = computed(() => !ios && wide.value);
const title = computed(() => (step.value === "tz" ? "Timezone" : edit.value ? "Edit Schedule" : "New Schedule"));
const saveLabel = computed(() => (ios ? (edit.value ? "Done" : "Add") : "Save"));
const machine = computed(() => machines.machines.find((m) => m.id === draft.machine_id));
const noAgent = computed(() => !!machine.value && !machine.value.agent);
// A power rule that is off can't be switched on without the agent (the server would reject it), as in MachineSchedules.
const enableLocked = computed(() => noAgent.value && draft.action !== "wake" && !draft.enabled);
const repeat = computed(() => {
  const kind = draft.kind;
  if (kind === "once") return { kind, date: draft.date };
  if (kind === "weekly") return { kind, days: DAYS.filter((d) => draft.days.includes(d)) };
  return { kind };
});
const clash = computed(() => {
  if (!draft.enabled || !draft.time || (draft.kind === "once" && !draft.date)) return null;
  const c = clashes(schedState.list, { id: edit.value && edit.value.id, machine_id: draft.machine_id, time: draft.time, repeat: repeat.value });
  return c.length ? c[0] : null;
});
const rowCls = ["item-content item-input", ios ? "" : "item-input-outline"];

watch(
  () => formState.seq,
  async (seq) => {
    if (!formState.open) return;
    try {
      await reloadSchedules();
    } catch {}
    if (seq !== formState.seq) return; // opened again meanwhile: that call fills the form
    const st = schedState.settings;
    const today = todayIn(st && st.timezone);
    const s = formState.schedule;
    Object.assign(draft, {
      machine_id: formState.machineId || (machines.machines[0] && machines.machines[0].id) || "",
      action: s ? s.action : "wake",
      time: s ? s.time : "08:00",
      kind: s ? s.repeat.kind : "daily",
      date: (s && s.repeat.date) || today,
      days: (s && s.repeat.days && [...s.repeat.days]) || [weekdayOf(today)],
      enabled: s ? s.enabled : true,
    });
    err.value = "";
    busy.value = false;
    result = false;
    step.value = !s && st && st.timezone == null ? "tz" : "form";
    shownSeq = seq;
    opened.value = true;
  },
);
// A new rule for a machine without an agent can only wake it.
watch(noAgent, (v) => {
  if (v && !edit.value) draft.action = "wake";
});

function toggleDay(d) {
  const i = draft.days.indexOf(d);
  if (i < 0) draft.days.push(d);
  else if (draft.days.length > 1) draft.days.splice(i, 1); // at least one day
}
const cap = (s) => s.charAt(0).toUpperCase() + s.slice(1);

async function useZone(name) {
  if (!name || busy.value) return;
  err.value = "";
  busy.value = true;
  try {
    await setTimezone(name);
    draft.date = todayIn(name);
    step.value = "form";
  } catch (e) {
    err.value = cap(offlineMsg(e));
  }
  busy.value = false;
}

async function done() {
  result = true;
  await Promise.all([reloadSchedules().catch(() => {}), refresh()]);
  opened.value = false;
}

async function save() {
  if (busy.value) return;
  err.value = "";
  if (!draft.time) return (err.value = "Pick a time.");
  if (draft.kind === "once" && !draft.date) return (err.value = "Pick a date.");
  busy.value = true;
  try {
    const s = edit.value;
    if (s) {
      // Send only what changed: the server re-checks "time already passed" when the timing changes.
      const patch = {};
      if (draft.action !== s.action) patch.action = draft.action;
      if (draft.time !== s.time) patch.time = draft.time;
      if (JSON.stringify(repeat.value) !== JSON.stringify(s.repeat)) patch.repeat = repeat.value;
      if (draft.enabled !== s.enabled) patch.enabled = draft.enabled;
      if (Object.keys(patch).length) await updateSchedule(s.id, patch);
    } else {
      const { machine_id, action, time, enabled } = draft;
      await createSchedule({ machine_id, action, time, repeat: repeat.value, enabled });
    }
    await done();
  } catch (e) {
    err.value = cap(offlineMsg(e));
  }
  busy.value = false;
}

async function del() {
  const ok = await confirmDialog({ title: "Delete this schedule?", text: "", label: "Delete", red: true });
  if (!ok) return;
  try {
    await deleteSchedule(edit.value.id);
    await done();
  } catch (e) {
    err.value = cap(offlineMsg(e));
  }
}

const cancel = () => (opened.value = false);
// iOS centres the navbar title by measuring it: again once the popup is shown with its new title.
const sizeNavbar = (p) => nextTick(() => p.el.querySelectorAll(".navbar").forEach((n) => f7.navbar.size(n)));
const onClosed = () => closeScheduleForm(shownSeq, result);
</script>

<template>
  <f7-popup v-model:opened="opened" class="zw-sched-form" swipe-to-close :push="ios" @popup:open="sizeNavbar" @popup:closed="onClosed">
    <div class="page">
      <!-- keyed: f7-navbar decides once whether it has a right side, and the timezone step has none -->
      <f7-navbar v-if="!dialog" :key="step" :title="title">
        <template #left>
          <f7-link v-if="ios" @click="cancel">Cancel</f7-link>
          <f7-link v-else icon-only aria-label="Close" @click="cancel"><Icon name="close" /></f7-link>
        </template>
        <template v-if="step === 'form'" #right>
          <f7-link :class="{ disabled: busy }" @click="save"><b v-if="ios">{{ saveLabel }}</b><template v-else>{{ saveLabel }}</template></f7-link>
        </template>
      </f7-navbar>
      <!-- Material wide (M3 dialog): Cancel / Save pinned at the bottom right by F7's bottom toolbar -->
      <div v-if="dialog" class="toolbar toolbar-bottom">
        <div class="toolbar-inner justify-content-flex-end">
          <a class="button" href="#" @click.prevent="cancel">Cancel</a>
          <a v-if="step === 'form'" class="button" :class="{ disabled: busy }" href="#" @click.prevent="save">Save</a>
        </div>
      </div>
      <div class="page-content">
        <div v-if="dialog" class="block-title block-title-large">{{ title }}</div>

        <template v-if="step === 'tz'">
          <ZonePicker :busy="busy" @pick="useZone" />
        </template>

        <form v-else @submit.prevent="save">
          <div class="list list-strong inset list-dividers-ios" :class="{ 'zw-inline': ios }">
            <ul>
              <li :class="[rowCls, { disabled: formState.fixed }]">
                <div class="item-inner">
                  <div class="item-title item-label"><label for="s-machine">Machine</label></div>
                  <div class="item-input-wrap" :class="{ 'input-dropdown-wrap': !formState.fixed }">
                    <select id="s-machine" v-model="draft.machine_id" :disabled="formState.fixed">
                      <option v-for="m in machines.machines" :key="m.id" :value="m.id">{{ m.name }}</option>
                    </select>
                  </div>
                </div>
              </li>
              <li class="zw-seg-row">
                <div class="segmented" :class="{ 'segmented-strong': ios }" role="radiogroup" aria-label="Action">
                  <button
                    v-for="(label, a) in ACTION"
                    :key="a"
                    class="button"
                    :class="{ 'button-active': draft.action === a, disabled: a !== 'wake' && noAgent }"
                    type="button"
                    role="radio"
                    :aria-checked="draft.action === a"
                    :disabled="a !== 'wake' && noAgent"
                    @click="draft.action = a"
                  >
                    <span class="dot" :class="a" /><span>{{ label }}</span>
                  </button>
                  <span v-if="ios" class="segmented-highlight" />
                </div>
              </li>
            </ul>
          </div>
          <div v-if="noAgent" class="block-footer">Reboot and Power off need the zerowol agent on this machine.</div>

          <div class="list list-strong inset list-dividers-ios" :class="{ 'zw-inline': ios }">
            <ul>
              <li :class="rowCls">
                <div class="item-inner">
                  <div class="item-title item-label"><label for="s-time">Time</label></div>
                  <div class="item-input-wrap"><input id="s-time" v-model="draft.time" type="time" required /></div>
                </div>
              </li>
              <li :class="rowCls">
                <div class="item-inner">
                  <div class="item-title item-label"><label for="s-repeat">Repeat</label></div>
                  <div class="item-input-wrap input-dropdown-wrap">
                    <select id="s-repeat" v-model="draft.kind">
                      <option v-for="(label, k) in REPEAT" :key="k" :value="k">{{ label }}</option>
                    </select>
                  </div>
                </div>
              </li>
              <li v-if="draft.kind === 'once'" :class="rowCls">
                <div class="item-inner">
                  <div class="item-title item-label"><label for="s-date">Date</label></div>
                  <div class="item-input-wrap"><input id="s-date" v-model="draft.date" type="date" min="1970-01-01" max="2999-12-31" required /></div>
                </div>
              </li>
              <li v-if="draft.kind === 'weekly'" class="zw-days" role="group" aria-label="Days">
                <button
                  v-for="d in DAYS"
                  :key="d"
                  class="button button-round"
                  :class="draft.days.includes(d) ? 'button-fill' : ios ? 'button-tonal' : 'button-outline'"
                  type="button"
                  :aria-label="DAY_LABEL[d]"
                  :aria-pressed="draft.days.includes(d)"
                  @click="toggleDay(d)"
                >
                  {{ DAY_LABEL[d][0] }}
                </button>
              </li>
            </ul>
          </div>

          <div class="list list-strong inset list-dividers-ios">
            <ul>
              <li class="item-content">
                <div class="item-inner">
                  <div class="item-title"><label for="s-enabled">Enabled</label></div>
                  <div class="item-after">
                    <label class="toggle" :class="{ disabled: enableLocked }">
                      <input id="s-enabled" v-model="draft.enabled" type="checkbox" :disabled="enableLocked" />
                      <span class="toggle-icon" />
                    </label>
                  </div>
                </div>
              </li>
            </ul>
          </div>
          <button type="submit" hidden />
        </form>

        <div v-if="step === 'form' && clash && machine" class="block block-strong inset" role="status">
          Another schedule acts on {{ machine.name }} at {{ clash.time }}. The second action may be rejected.
        </div>
        <div class="block errmsg" role="alert">{{ err }}</div>

        <template v-if="step === 'form' && edit">
          <div v-if="ios" class="list list-strong inset list-dividers-ios">
            <ul>
              <li><a class="list-button color-red" href="#" @click.prevent="del">Delete Schedule</a></li>
            </ul>
          </div>
          <div v-else class="block">
            <button class="button button-outline color-red" type="button" @click="del">Delete Schedule</button>
          </div>
        </template>

      </div>
    </div>
  </f7-popup>
</template>
