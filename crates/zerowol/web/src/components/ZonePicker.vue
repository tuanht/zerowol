<script setup>
import { zoneChoice } from "../lib/schedules.js";

// The timezone prompt shown while none is set: the form's first step and the Schedules page.
defineProps({ busy: Boolean });
// "more": the full, searchable list (the form shows it as its next step, the Schedules page as a page).
const emit = defineEmits(["pick", "more"]);
const { zone, offset } = zoneChoice();
</script>

<template>
  <div class="block-title block-title-large text-align-center">{{ zone }}</div>
  <div class="block text-align-center">
    <p v-if="offset">({{ offset }})</p>
    <p>Schedules run on the server's clock. Use your timezone?</p>
  </div>
  <div class="block">
    <button class="button button-large button-fill button-round" :class="{ disabled: busy }" type="button" @click="emit('pick', zone)">Use {{ zone }}</button>
    <button class="button button-large" :class="{ disabled: busy }" type="button" @click="emit('more')">Choose another…</button>
  </div>
</template>
