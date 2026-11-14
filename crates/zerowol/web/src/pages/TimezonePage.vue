<script setup>
import { computed, ref } from "vue";
import ZoneList from "../components/ZoneList.vue";
import { offlineMsg } from "../lib/api.js";
import { schedState, setTimezone } from "../lib/schedules.js";
import { toast } from "../lib/ui.js";

// Settings → Timezone (and the Schedules page's "Choose another…"): search and pick the schedule timezone.
// `pick` replaces the plain save, e.g. so the Schedules page reloads after it.
const props = defineProps({ f7router: { type: Object, required: true }, pick: { type: Function, default: null } });
const query = ref("");
const current = computed(() => (schedState.settings && schedState.settings.timezone) || "");
let busy = false;

async function choose(zone) {
  if (busy) return;
  busy = true;
  try {
    await (props.pick || setTimezone)(zone);
    props.f7router.back();
  } catch (e) {
    toast(offlineMsg(e));
  }
  busy = false;
}
</script>

<template>
  <!-- with-subnavbar up front: detected later, f7-page would re-render its class and drop the router's page-current -->
  <f7-page name="timezone" with-subnavbar>
    <f7-navbar title="Timezone" back-link :back-link-show-text="false">
      <f7-subnavbar :inner="false">
        <f7-searchbar custom-search :backdrop="false" @searchbar:search="(sb, q) => (query = q)" @searchbar:clear="query = ''" />
      </f7-subnavbar>
    </f7-navbar>
    <ZoneList :query="query" :current="current" @pick="choose" />
  </f7-page>
</template>
