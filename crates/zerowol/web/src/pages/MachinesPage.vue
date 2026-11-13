<script setup>
import { inject } from "vue";
import { theme } from "framework7-vue/bundle";
import Icon from "../components/Icon.vue";
import MachineCard from "../components/MachineCard.vue";
import { state } from "../lib/machines.js";

const addUrl = theme.ios ? inject("addFor")["view-machines"] : null;
</script>

<template>
  <f7-page name="machines">
    <f7-navbar title="Machines" large>
      <f7-nav-right v-if="addUrl">
        <f7-link :href="addUrl" icon-only aria-label="Add Machine"><Icon name="plus" /></f7-link>
      </f7-nav-right>
    </f7-navbar>
    <div v-if="state.loaded && !state.machines.length" class="list inset media-list">
      <ul>
        <li>
          <a class="item-link item-content empty" href="/machine/new/">
            <div class="item-inner">
              <div><b>Add your first machine</b><div class="sub">Tap to set up Wake-on-LAN</div></div>
            </div>
          </a>
        </li>
      </ul>
    </div>
    <div v-else class="zw-grid">
      <MachineCard v-for="m in state.machines" :key="m.id" :machine="m" />
    </div>
    <f7-block-footer v-if="state.machines.length" class="foot">
      Wake uses a magic packet. Reboot and Power off require the zerowol agent.
    </f7-block-footer>
  </f7-page>
</template>
