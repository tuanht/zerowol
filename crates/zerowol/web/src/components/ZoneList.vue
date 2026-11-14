<script setup>
import { computed } from "vue";
import { browserZone } from "../lib/schedules.js";
import { modernZone, searchZones, zoneEntries } from "../lib/zones.js";

// The timezone list under a searchbar (Settings → Timezone, the form's "Choose another…" step).
// Empty query: Suggested (this device, the current zone, UTC), then every zone by region.
// Otherwise one ranked list. Offsets and clock times are taken once, when the list is shown.
const props = defineProps({ query: { type: String, default: "" }, current: { type: String, default: "" } });
const emit = defineEmits(["pick"]);

const entries = zoneEntries(props.current);
const byId = new Map(entries.map((e) => [e.id, e]));
const selected = computed(() => props.current && modernZone(props.current));
const suggested = [...new Set([browserZone(), selected.value, "UTC"])].map((id) => byId.get(id)).filter(Boolean);
const regions = [];
for (const e of entries) {
  const last = regions[regions.length - 1];
  if (last && last.name === e.region) last.zones.push(e);
  else regions.push({ name: e.region, zones: [e] });
}
const found = computed(() => (props.query.trim() ? searchZones(entries, props.query) : null));
// Without a query: Suggested, then the regions; with one: a single untitled section.
const sections = computed(() =>
  found.value ? [{ name: "", zones: found.value }] : [{ name: "Suggested", zones: suggested }, ...regions],
);
</script>

<template>
  <div v-if="found && !found.length" class="block">
    <p>No matching timezones</p>
  </div>
  <template v-for="s in sections" :key="s.name">
    <div v-if="s.name" class="block-title">{{ s.name }}</div>
    <div v-if="s.zones.length" class="list list-strong inset list-dividers-ios zw-zones">
      <ul>
        <li v-for="e in s.zones" :key="e.id">
          <!-- one radio group per section, so the current zone is checked in Suggested and in its region -->
          <label class="item-radio item-content">
            <!-- .prevent: the check moves once the zone is saved (the `current` prop), not on tap -->
            <input type="radio" :name="'zw-zone-' + s.name" :value="e.id" :checked="e.id === selected" @click.prevent="emit('pick', e.id)" />
            <i class="icon icon-radio" />
            <div class="item-inner">
              <div class="item-title">
                {{ e.city }}
                <div v-if="e.id !== e.city" class="item-footer">{{ e.id }}</div>
              </div>
              <div class="item-after">{{ e.offset }} · {{ e.time }}</div>
            </div>
          </label>
        </li>
      </ul>
    </div>
  </template>
</template>
