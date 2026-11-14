<script setup>
import { zoneChoice } from "../lib/schedules.js";

// The timezone prompt shown while none is set: the form's first step and the Schedules page.
defineProps({ busy: Boolean });
const emit = defineEmits(["pick"]);
const { zone, offset, groups } = zoneChoice();
</script>

<template>
  <div class="block-title block-title-large text-align-center">{{ zone }}</div>
  <div class="block text-align-center">
    <p v-if="offset">({{ offset }})</p>
    <p>Schedules run on the server's clock. Use your timezone?</p>
  </div>
  <div class="block">
    <button class="button button-large button-fill button-round" :class="{ disabled: busy }" type="button" @click="emit('pick', zone)">Use {{ zone }}</button>
    <label class="button button-large zw-pick">
      Choose another…
      <select aria-label="Timezone" @change="emit('pick', $event.target.value)">
        <option value="" selected disabled>Choose another…</option>
        <optgroup v-for="g in groups" :key="g.label" :label="g.label">
          <option v-for="z in g.zones" :key="z" :value="z">{{ z }}</option>
        </optgroup>
      </select>
    </label>
  </div>
</template>
