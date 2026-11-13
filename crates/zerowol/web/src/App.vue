<script setup>
import { computed, provide, ref } from "vue";
import { f7ready, theme } from "framework7-vue/bundle";
import Icon from "./components/Icon.vue";
import MachinesPage from "./pages/MachinesPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import { applyScheme } from "./lib/prefs.js";
import { installBackGuard } from "./lib/back.js";

const props = defineProps({ f7params: { type: Object, required: true } });

// One entry per tab, in display order. Each tab is its own view with its own
// page stack. Phase 2 inserts Schedules between Machines and Settings.
const tabs = [
  { id: "view-machines", label: "Machines", icon: "desk", url: "/" },
  { id: "view-settings", label: "Settings", icon: "gear", url: "/settings/" },
];
// The add action of each tab: iOS pages show it as "＋" in their navbar,
// Material shows it as the FAB (bottom right on phones, top of the rail when wide).
const addFor = { "view-machines": "/machine/new/" };
provide("addFor", addFor);

const params = {
  ...props.f7params,
  routes: [
    { path: "/", component: MachinesPage },
    { path: "/settings/", component: SettingsPage },
  ],
};

const IOS = !!theme.ios;
const root = document.documentElement;
root.classList.add(IOS ? "zw-ios" : "zw-md");

// "Wide" (tablet/desktop) switches the navigation layout live, e.g. on rotation.
const wideQuery = matchMedia("(min-width: 768px)");
const setWide = () => root.classList.toggle("zw-wide", wideQuery.matches);
setWide();
wideQuery.addEventListener("change", setWide);

const activeTab = ref(tabs[0].id);
const fabUrl = computed(() => (IOS ? null : addFor[activeTab.value]));
let f7app = null;
const viewOf = (id) => f7app && f7app.views.get("#" + id);

f7ready((f7) => {
  f7app = f7;
  applyScheme(f7);
  f7.on("tabShow", (el) => {
    if (el && el.id) activeTab.value = el.id;
  });
  if (!IOS) installBackGuard(f7, () => viewOf(activeTab.value));
});

function add() {
  const v = viewOf(activeTab.value);
  if (v && fabUrl.value) v.router.navigate(fabUrl.value);
}
</script>

<template>
  <f7-app v-bind="params">
    <f7-views tabs class="safe-areas">
      <f7-toolbar tabbar icons bottom class="zw-nav">
        <f7-link
          v-for="(t, i) in tabs"
          :key="t.id"
          :tab-link="'#' + t.id"
          :tab-link-active="i === 0"
          :aria-label="t.label"
        >
          <i class="icon"><Icon :name="t.icon" /></i>
          <span class="tabbar-label">{{ t.label }}</span>
        </f7-link>
      </f7-toolbar>
      <f7-view
        v-for="(t, i) in tabs"
        :id="t.id"
        :key="t.id"
        :main="i === 0"
        tab
        :tab-active="i === 0"
        :url="t.url"
      />
      <div v-if="fabUrl" class="fab fab-right-bottom zw-fab">
        <a href="#" aria-label="Add" @click.prevent="add"><Icon name="plus" /></a>
      </div>
    </f7-views>
  </f7-app>
</template>
