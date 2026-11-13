<script setup>
import { inject } from "vue";
import { theme } from "framework7-vue/bundle";
import Icon from "../components/Icon.vue";

const addUrl = theme.ios ? inject("addFor")["view-machines"] : null;
</script>

<template>
  <f7-page name="machines">
    <f7-navbar title="Machines" large>
      <f7-nav-right v-if="addUrl">
        <f7-link :href="addUrl" icon-only aria-label="Add Machine"><Icon name="plus" /></f7-link>
      </f7-nav-right>
    </f7-navbar>
  </f7-page>
</template>
