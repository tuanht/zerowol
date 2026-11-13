<script setup>
import { computed, provide, ref } from "vue";
import { f7ready, theme } from "framework7-vue/bundle";
import Icon from "./components/Icon.vue";
import MachinesPage from "./pages/MachinesPage.vue";
import MachinePage from "./pages/MachinePage.vue";
import PairPage from "./pages/PairPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import { applyScheme } from "./lib/prefs.js";
import { installBackGuard } from "./lib/back.js";
import { state as machines, startPolling } from "./lib/machines.js";
import { api, offlineMsg } from "./lib/api.js";
import { toast } from "./lib/ui.js";

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

// Finds the machine in the store, or fetches the list once (deep link) and
// rejects with a toast when it does not exist.
function machineRoute(component) {
  const done = (resolve, machine) => resolve({ component }, { props: { machine } });
  return async ({ to, resolve, reject }) => {
    const id = to.params.id;
    const m = machines.machines.find((x) => x.id === id);
    if (m) return done(resolve, m);
    try {
      machines.machines = await api("GET", "/machines");
      machines.loaded = true;
      const found = machines.machines.find((x) => x.id === id);
      if (found) return done(resolve, found);
      toast("Machine not found");
    } catch (e) {
      toast(offlineMsg(e));
    }
    reject();
  };
}

const params = {
  ...props.f7params,
  routes: [
    { path: "/", component: MachinesPage },
    { path: "/machine/new/", component: MachinePage },
    { path: "/machine/:id/pair/", async: machineRoute(PairPage) },
    { path: "/machine/:id/", async: machineRoute(MachinePage) },
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
// The add FAB belongs to a tab's root page only, not to its sub-pages.
const atRoot = ref(true);
const fabUrl = computed(() => (IOS || !atRoot.value ? null : addFor[activeTab.value]));
let f7app = null;
const viewOf = (id) => f7app && f7app.views.get("#" + id);
const syncRoot = () => {
  const v = viewOf(activeTab.value);
  atRoot.value = !v || !v.router || v.router.history.length <= 1;
};

f7ready((f7) => {
  f7app = f7;
  applyScheme(f7);
  startPolling();
  f7.on("tabShow", (el) => {
    if (el && el.id) activeTab.value = el.id;
    syncRoot();
  });
  f7.on("routeChanged", syncRoot);
  f7.on("pageAfterIn", syncRoot);
  if (!IOS) installBackGuard(f7, () => viewOf(activeTab.value));
});

function add() {
  const v = viewOf(activeTab.value);
  if (v && fabUrl.value) v.router.navigate(fabUrl.value);
}
</script>

<template>
  <f7-app v-bind="params">
    <div v-if="machines.offline" class="offline" role="alert">Can't reach zerowol</div>
    <f7-views tabs class="safe-areas">
      <f7-toolbar tabbar icons bottom class="zw-nav">
        <!-- toolbar-pane gives the iOS Liquid Glass pill (display:contents on Material) -->
        <f7-toolbar-pane>
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
        </f7-toolbar-pane>
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
