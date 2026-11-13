<script setup>
import { computed } from "vue";
import { f7, theme } from "framework7-vue/bundle";
import Icon from "./Icon.vue";
import { inflight, power, wake } from "../lib/machines.js";
import { ACTION, BUSY } from "../lib/words.js";
import { confirmDialog } from "../lib/ui.js";

const props = defineProps({ machine: { type: Object, required: true } });
const IOS = !!theme.ios;

const t = computed(() => inflight.get(props.machine.id));
const st = computed(() => props.machine.status || {});
const on = computed(() => !!st.value.online);
const label = computed(() => (t.value ? BUSY[t.value.kind] : on.value ? "Online" : "Offline"));
const meta = computed(() => {
  const s = st.value;
  return (
    props.machine.ip +
    " · " +
    (on.value ? (s.uptime != null ? "Up " + uptime(s.uptime) : "Online") : "Offline") +
    (s.agent_error ? " · Agent: " + s.agent_error : "")
  );
});
const mode = computed(() => {
  const m = props.machine;
  if (on.value && st.value.agent === "paired") return "power";
  if (m.agent && st.value.agent_error) return "err";
  if (on.value && m.agent) return "unreach";
  return "wake";
});

function uptime(s) {
  const d = Math.floor(s / 86400), hr = Math.floor(s / 3600) % 24, m = Math.floor(s / 60) % 60;
  return d ? d + "d " + hr + "h" : hr ? hr + "h " + m + "m" : m ? m + "m" : "<1m";
}
const pair = () => f7.views.current.router.navigate("/machine/" + props.machine.id + "/pair/");

async function confirmPower(kind) {
  const n = props.machine.name;
  const off = kind === "poweroff";
  const ok = await confirmDialog({
    title: (off ? "Power off " : "Reboot ") + n + "?",
    text: off ? "Unsaved work on " + n + " will be lost." : n + " will reboot. Unsaved work will be lost.",
    label: ACTION[kind],
    red: off,
  });
  if (ok) power(props.machine, kind);
}
</script>

<template>
  <div class="card machine" :class="{ 'card-outline': !IOS }">
    <div class="list media-list no-margin no-hairlines">
      <ul>
        <li>
          <a class="item-link item-content" :href="'/machine/' + machine.id + '/'">
            <div class="item-media"><div class="tile"><Icon name="desk" /></div></div>
            <div class="item-inner">
              <div class="item-title-row">
                <div class="item-title name">{{ machine.name }}</div>
                <div class="item-after status-r" :class="{ 'busy-t': t }">
                  <span class="dot" :class="t ? 'busy' : on ? 'on' : ''" />{{ label }}
                </div>
              </div>
              <div class="item-subtitle meta">{{ meta }}</div>
            </div>
          </a>
        </li>
      </ul>
    </div>
    <div class="card-footer actions">
      <template v-if="mode === 'power'">
        <button class="button" type="button" :class="{ disabled: !!t }" :disabled="!!t" @click="confirmPower('reboot')">
          <Icon name="redo" /><span>{{ ACTION.reboot }}</span>
        </button>
        <button class="button color-red" type="button" :class="{ disabled: !!t }" :disabled="!!t" @click="confirmPower('poweroff')">
          <Icon name="power" /><span>{{ ACTION.poweroff }}</span>
        </button>
      </template>
      <button v-else-if="mode !== 'wake'" class="button" type="button" :class="{ disabled: !!t }" :disabled="!!t" @click="pair">
        <Icon name="redo" /><span>{{ mode === "err" ? "Agent: " + st.agent_error : "Agent unreachable" }} · Pair</span>
      </button>
      <button v-else class="button" type="button" :class="{ disabled: !!t }" :disabled="!!t" @click="wake(machine)">
        <Icon name="power" /><span>{{ ACTION.wake }}</span>
      </button>
      <slot name="trailing" />
    </div>
  </div>
</template>
