<script setup>
import { ACTION, REPEAT } from "../lib/words.js";
import { repeatLabel, schedState } from "../lib/schedules.js";
import { outcomeText } from "../lib/occurrences.js";

const props = defineProps({
  occurrences: { type: Array, required: true },
  now: { type: Number, required: true }, // unix seconds
});
const emit = defineEmits(["pick"]);

const repeatOf = (o) => {
  const s = schedState.list.find((x) => x.id === o.schedule_id);
  return s ? repeatLabel(s.repeat) : REPEAT[o.repeat_kind] || o.repeat_kind;
};
// Greyed when the rule is off, except an occurrence that already ran (a once rule switches itself off after firing).
const off = (o) => !o.enabled && !(o.outcome && o.at <= props.now);
const bad = (o) => /^(failed|missed)/.test(outcomeText(o, props.now));
</script>

<template>
  <div class="list list-strong inset list-dividers-ios">
    <ul>
      <li v-for="o in occurrences" :key="o.schedule_id + o.date">
        <a class="item-link item-content" href="#" @click.prevent="emit('pick', o, $event.currentTarget)">
          <div class="item-inner">
            <div class="item-title">
              <component :is="o.skipped ? 's' : 'span'" :class="{ 'zw-off': off(o) }">{{ o.time }} · {{ o.machine_name }}</component>
              <div class="item-footer">
                {{ repeatOf(o) }}<template v-if="off(o)"> · off</template>
                <template v-if="outcomeText(o, now)"> · <span :class="{ 'zw-bad': bad(o) }">{{ outcomeText(o, now) }}</span></template>
              </div>
            </div>
            <div class="item-after"><span class="zw-pill" :class="off(o) ? 'off' : o.action">{{ ACTION[o.action] }}</span></div>
          </div>
        </a>
      </li>
      <li v-if="!occurrences.length" class="item-content">
        <div class="item-inner"><div class="item-title"><span class="zw-off">Nothing scheduled</span></div></div>
      </li>
    </ul>
  </div>
</template>
