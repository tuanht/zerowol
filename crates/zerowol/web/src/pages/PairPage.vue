<script setup>
import { onBeforeUnmount, ref } from "vue";
import { api, offlineMsg } from "../lib/api.js";
import { refresh } from "../lib/machines.js";
import { confirmDialog, copyText, toast } from "../lib/ui.js";

const props = defineProps({ f7router: { type: Object, required: true }, machine: { type: Object, required: true } });
const m = props.machine;
const cmd = ref("…");
const paired = ref(false);
const wait = ref("Waiting for agent…");
const copied = ref(false);
let alive = true;
let gen = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const stop = () => (alive = false);
onBeforeUnmount(stop);

// Rotation happens in place (never via a URL), so history back/forward can't
// silently regenerate the secret.
async function load(rotate) {
  const my = ++gen;
  cmd.value = "…";
  paired.value = false;
  wait.value = "Waiting for agent…";
  try {
    const r = await api(rotate ? "POST" : "GET", "/machines/" + m.id + "/agent/" + (rotate ? "rotate" : "install-command"));
    if (my === gen) cmd.value = r.install_command;
  } catch (e) {
    toast(offlineMsg(e));
  }
  while (alive && my === gen) {
    await sleep(2000);
    if (!alive || my !== gen) return;
    try {
      const s = await api("GET", "/machines/" + m.id + "/status");
      if (!alive || my !== gen) return;
      if (s.agent !== "paired") wait.value = s.agent_error ? "Agent: " + s.agent_error : "Waiting for agent…";
      else {
        paired.value = true;
        refresh();
        return;
      }
    } catch {}
  }
}
load(false);

async function copy() {
  await copyText(cmd.value);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
async function regen() {
  const ok = await confirmDialog({
    title: "Regenerate secret?",
    text: "The installed agent will stop working until you run the new command.",
    label: "Regenerate",
    red: true,
  });
  if (ok) load(true);
}
</script>

<template>
  <f7-page name="pair" @page:beforeremove="stop">
    <f7-navbar :title="'Pair ' + m.name" back-link :back-link-show-text="false">
      <f7-nav-right><f7-link @click="props.f7router.back()">Done</f7-link></f7-nav-right>
    </f7-navbar>
    <div class="block-title">Install command</div>
    <div class="block block-strong inset">
      <p>Run this command on {{ m.name }} (Linux).</p>
      <div class="code">{{ cmd }}</div>
      <div class="pair-btn">
        <button class="button button-large button-fill" type="button" @click="copy">{{ copied ? "Copied" : "Copy" }}</button>
      </div>
      <div class="pair" :class="{ ok: paired }">
        <template v-if="paired">Paired ✓</template>
        <template v-else><span class="dot busy" /><span>{{ wait }}</span></template>
      </div>
    </div>
    <div class="list list-strong inset list-dividers-ios">
      <ul><li><a class="list-button color-red" href="#" @click.prevent="regen">Regenerate secret</a></li></ul>
    </div>
    <div class="block-footer">Regenerating invalidates the secret the agent currently uses.</div>
  </f7-page>
</template>
