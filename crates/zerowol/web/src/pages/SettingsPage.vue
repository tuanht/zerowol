<script setup>
import { onMounted, ref } from "vue";
import { f7 } from "framework7-vue/bundle";
import { api, offlineMsg } from "../lib/api.js";
import { THEME_KEY, SCHEME_KEY, themePref, schemePref, setPref, applyScheme } from "../lib/prefs.js";
import { confirmDialog, copyText, toast } from "../lib/ui.js";

const version = __VERSION__;
const REPO = "https://github.com/tuanht/zerowol";

const theme = ref(themePref());
const scheme = ref(schemePref());
const tokens = ref([]);
const name = ref("");
const fresh = ref(null);
const copied = ref(false);

const themes = [
  ["auto", "Automatic", "Match this device"],
  ["ios", "iOS"],
  ["md", "Android"],
];
const schemes = [
  ["auto", "Automatic", "Follow system light/dark setting"],
  ["light", "Light"],
  ["dark", "Dark"],
];

function setTheme(v) {
  theme.value = v;
  setPref(THEME_KEY, v);
  location.reload();
}
function setScheme(v) {
  scheme.value = v;
  setPref(SCHEME_KEY, v);
  applyScheme(f7);
}

const date = (s) => new Date(s * 1000).toLocaleDateString();

async function load() {
  try {
    tokens.value = await api("GET", "/tokens");
  } catch (e) {
    toast(offlineMsg(e));
  }
}
onMounted(load);

async function create() {
  try {
    fresh.value = await api("POST", "/tokens", { name: name.value.trim() });
    name.value = "";
    copied.value = false;
    load();
  } catch (e) {
    toast(offlineMsg(e));
  }
}
async function revoke(t) {
  const ok = await confirmDialog({
    title: "Revoke " + t.name + "?",
    text: "Anything using this token will stop working.",
    label: "Revoke",
    red: true,
  });
  if (!ok) return;
  try {
    await api("DELETE", "/tokens/" + t.id);
  } catch (e) {
    toast(offlineMsg(e));
  }
  load();
}
async function copy() {
  await copyText(fresh.value.token);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
</script>

<template>
  <f7-page name="settings">
    <f7-navbar title="Settings" large />

    <div class="block-title">Theme</div>
    <div class="list list-strong inset list-dividers-ios">
      <ul>
        <li v-for="[v, t, sub] in themes" :key="v">
          <label class="item-radio item-radio-icon-start item-content">
            <input type="radio" name="zw-theme" :value="v" :checked="theme === v" @change="setTheme(v)" />
            <i class="icon icon-radio" />
            <div class="item-inner">
              <div class="item-title">{{ t }}<div v-if="sub" class="item-footer">{{ sub }}</div></div>
            </div>
          </label>
        </li>
      </ul>
    </div>

    <div class="block-title">Appearance</div>
    <div class="list list-strong inset list-dividers-ios">
      <ul>
        <li v-for="[v, t, sub] in schemes" :key="v">
          <label class="item-radio item-radio-icon-start item-content">
            <input type="radio" name="zw-scheme" :value="v" :checked="scheme === v" @change="setScheme(v)" />
            <i class="icon icon-radio" />
            <div class="item-inner">
              <div class="item-title">{{ t }}<div v-if="sub" class="item-footer">{{ sub }}</div></div>
            </div>
          </label>
        </li>
      </ul>
    </div>

    <div class="block-title">API Tokens</div>
    <div v-if="fresh" class="block block-strong inset new-token">
      <p>Copy {{ fresh.name }} now. It won't be shown again.</p>
      <div class="code">{{ fresh.token }}</div>
      <div class="pair-btn">
        <button class="button button-large button-fill button-round" type="button" @click="copy">{{ copied ? "Copied" : "Copy" }}</button>
      </div>
    </div>
    <div class="list list-strong inset list-dividers-ios">
      <ul>
        <li v-for="t in tokens" :key="t.id" class="item-content">
          <div class="item-inner">
            <div class="item-title">
              {{ t.name }}
              <div class="item-footer">Created {{ date(t.created) }} · {{ t.last_used ? "Used " + date(t.last_used) : "Never used" }}</div>
            </div>
            <div class="item-after"><a class="link color-red" href="#" @click.prevent="revoke(t)">Revoke</a></div>
          </div>
        </li>
        <li class="item-content item-input token-row">
          <div class="item-inner">
            <div class="item-input-wrap">
              <input v-model="name" type="text" placeholder="Token name" autocapitalize="off" autocomplete="off" maxlength="64" @keydown.enter.prevent="create" />
            </div>
            <div class="item-after"><a class="link" href="#" @click.prevent="create">Create</a></div>
          </div>
        </li>
      </ul>
    </div>
    <div class="block-footer">Tokens let scripts call the API with Authorization: Bearer.</div>

    <div class="block-title">About</div>
    <div class="list list-strong inset list-dividers-ios">
      <ul>
        <li class="item-content">
          <div class="item-inner"><div class="item-title">Version</div><div class="item-after">{{ version }}</div></div>
        </li>
        <li>
          <a class="item-link item-content external" :href="REPO" target="_blank" rel="noopener">
            <div class="item-inner"><div class="item-title">zerowol on GitHub</div></div>
          </a>
        </li>
      </ul>
    </div>
  </f7-page>
</template>
