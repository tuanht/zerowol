<script setup>
import { computed, nextTick, ref } from "vue";
import { f7 } from "framework7-vue";
import { IOS } from "../lib/layout.js";
import { offlineMsg } from "../lib/api.js";
import { toast } from "../lib/ui.js";
import { refresh, state as machines } from "../lib/machines.js";
import { ACTION } from "../lib/words.js";
import Icon from "./Icon.vue";
import { openScheduleForm, reloadSchedules, repeatLabel, schedState, sheetState, updateSchedule } from "../lib/schedules.js";

const ios = IOS;
const busy = ref("");
const machine = computed(() => machines.machines.find((m) => m.id === sheetState.machineId));
const rules = computed(() =>
  schedState.list.filter((s) => s.machine_id === sheetState.machineId).sort((a, b) => (a.time < b.time ? -1 : a.time > b.time ? 1 : 0)),
);
// A power rule can't be switched on without the agent (the server would reject it too).
const needsAgent = (s) => s.action !== "wake" && !(machine.value && machine.value.agent);

async function toggle(s, ev) {
  const on = ev.target.checked;
  busy.value = s.id;
  try {
    await updateSchedule(s.id, { enabled: on });
  } catch (e) {
    ev.target.checked = !on;
    toast(offlineMsg(e));
  }
  busy.value = "";
  await Promise.all([reloadSchedules().catch(() => {}), refresh()]);
}

// iOS centres the navbar title by measuring it: again once the sheet is shown with this machine's name.
const sizeNavbar = (sh) => nextTick(() => sh.el.querySelectorAll(".navbar").forEach((n) => f7.navbar.size(n)));

// The form popup sits below sheets, so the sheet steps aside while the form is open and comes back after.
async function openForm(schedule) {
  const id = sheetState.machineId;
  sheetState.open = false;
  await openScheduleForm({ machineId: id, schedule });
  if (schedState.list.some((s) => s.machine_id === id)) {
    sheetState.machineId = id;
    sheetState.open = true;
  }
}
</script>

<template>
  <f7-sheet v-model:opened="sheetState.open" class="zw-sched-sheet" swipe-to-close backdrop style="height: auto" @sheet:open="sizeNavbar">
    <f7-navbar :title="machine ? machine.name : ''">
      <template #left><f7-link icon-only sheet-close aria-label="Close"><Icon name="close" /></f7-link></template>
    </f7-navbar>
    <div class="zw-sheet-body">
      <div class="list list-strong inset list-dividers-ios">
        <ul>
          <li v-for="s in rules" :key="s.id">
            <div class="item-content" role="button" tabindex="0" @click="openForm(s)" @keydown.enter="openForm(s)">
              <div class="item-inner">
                <div class="item-title">
                  {{ s.time }} · {{ repeatLabel(s.repeat) }}
                  <div v-if="!s.enabled && needsAgent(s)" class="item-footer">needs agent</div>
                </div>
                <div class="item-after">
                  <span class="zw-pill" :class="s.action">{{ ACTION[s.action] }}</span>
                  <label class="toggle" :class="{ disabled: busy === s.id || (!s.enabled && needsAgent(s)) }" @click.stop @keydown.enter.stop>
                    <input
                      type="checkbox"
                      :checked="s.enabled"
                      :disabled="busy === s.id || (!s.enabled && needsAgent(s))"
                      :aria-label="'Enabled ' + s.time"
                      @change="toggle(s, $event)"
                    />
                    <span class="toggle-icon" />
                  </label>
                </div>
              </div>
            </div>
          </li>
          <li><a class="list-button" href="#" @click.prevent="openForm()">Add Schedule</a></li>
        </ul>
      </div>
    </div>
  </f7-sheet>
</template>
