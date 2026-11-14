<script setup>
defineProps({ name: { type: String, required: true } });
</script>

<template>
  <svg class="ico" viewBox="0 0 24 24" aria-hidden="true">
    <template v-if="name === 'desk'"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></template>
    <template v-else-if="name === 'power'"><path d="M12 3v8" /><path d="M6.4 6.8a8 8 0 1 0 11.2 0" /></template>
    <template v-else-if="name === 'redo'"><path d="M20 12a8 8 0 1 1-2.5-5.8" /><path d="M20 4v4.5h-4.5" /></template>
    <template v-else-if="name === 'plus'"><path d="M12 5v14M5 12h14" /></template>
    <template v-else-if="name === 'gear'"><circle cx="12" cy="12" r="3" /><path d="M12 2.8l1.6 2.2 2.7-.5.9 2.6 2.6.9-.5 2.7 2.2 1.6-2.2 1.6.5 2.7-2.6.9-.9 2.6-2.7-.5L12 21.2 10.4 19l-2.7.5-.9-2.6-2.6-.9.5-2.7L2.5 12l2.2-1.6-.5-2.7 2.6-.9.9-2.6 2.7.5z" /></template>
    <template v-else-if="name === 'clock'"><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></template>
    <template v-else-if="name === 'calendar'"><rect x="3.5" y="5" width="17" height="15" rx="2" /><path d="M3.5 10h17M8 3v4M16 3v4" /></template>
    <template v-else-if="name === 'prev'"><path d="M15 5l-7 7 7 7" /></template>
    <template v-else-if="name === 'next'"><path d="M9 5l7 7-7 7" /></template>
    <template v-else-if="name === 'close'"><path d="M6 6l12 12M18 6L6 18" /></template>
  </svg>
</template>
