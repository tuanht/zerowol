<script setup>
import { computed, reactive, ref } from "vue";
import { f7, theme } from "framework7-vue/bundle";
import { api, offlineMsg } from "../lib/api.js";
import { inflight, refresh } from "../lib/machines.js";
import { confirmDialog, toast } from "../lib/ui.js";

const props = defineProps({ f7router: { type: Object, required: true }, machine: { type: Object, default: null } });
const IOS = !!theme.ios;
const m = props.machine;
const v = (x) => (x == null ? "" : String(x));
const form = reactive({
  name: v(m && m.name),
  mac: v(m && m.mac),
  ip: v(m && m.ip),
  broadcast: v(m && m.broadcast),
  wol_port: v(m && m.wol_port),
  probe_port: v(m && m.probe_port),
  agent: !!(m && m.agent),
});
const invalid = ref("");
const err = ref("");
const busy = ref(false);
const page = ref(null);
const adv = ref(null);

// iOS numeric pads show the locale decimal separator (often ","), so use the
// url keyboard (always has ".") and normalise any typed "," to ".".
const dots = (k) => (form[k] = form[k].replace(/,/g, "."));
const bcastPh = computed(() => {
  const o = form.ip.trim().split(".");
  return o.length === 4 ? o[0] + "." + o[1] + "." + o[2] + ".255" : "Default from IP";
});
const rowCls = (k) => ["item-content item-input", IOS ? "" : "item-input-outline", invalid.value === k ? "item-input-invalid" : ""];
const current = () => page.value && props.f7router.currentPageEl === page.value.$el;

async function save() {
  if (busy.value) return;
  invalid.value = "";
  err.value = "";
  const body = { name: form.name.trim(), mac: form.mac.trim(), ip: form.ip.trim(), agent: form.agent };
  if (form.broadcast.trim()) body.broadcast = form.broadcast.trim();
  if (form.wol_port !== "" && form.wol_port != null) body.wol_port = +form.wol_port;
  if (form.probe_port !== "" && form.probe_port != null) body.probe_port = +form.probe_port;
  busy.value = true;
  try {
    const r = m ? await api("PATCH", "/machines/" + m.id, body) : await api("POST", "/machines", body);
    await refresh();
    // The user may have gone back while the request was in flight.
    if (!current()) return;
    if (form.agent && !(m && m.agent)) props.f7router.navigate("/machine/" + r.id + "/pair/", { reloadCurrent: true });
    else props.f7router.back();
  } catch (e) {
    busy.value = false;
    const k = (/^invalid (name|mac|ip|broadcast|wol_port|probe_port)/.exec(e.message) || [])[1];
    if (k && /port$/.test(k) && adv.value) f7.accordion.open(adv.value);
    invalid.value = k || "";
    const msg = offlineMsg(e);
    err.value = msg.charAt(0).toUpperCase() + msg.slice(1);
  }
}

async function del() {
  const ok = await confirmDialog({
    title: "Delete " + m.name + "?",
    text: "This removes the machine from zerowol. The PC itself is not touched.",
    label: "Delete",
    red: true,
  });
  if (!ok) return;
  try {
    await api("DELETE", "/machines/" + m.id);
    inflight.delete(m.id);
    await refresh();
    if (current()) props.f7router.back();
  } catch (e) {
    toast(offlineMsg(e));
  }
}
</script>

<template>
  <f7-page ref="page" name="machine">
    <f7-navbar :title="m ? 'Edit Machine' : 'Add Machine'" back-link :back-link-show-text="false" />
    <form @submit.prevent="save">
      <div class="list list-strong inset list-dividers-ios" :class="{ 'zw-inline': IOS }">
        <ul>
          <li :class="rowCls('name')">
            <div class="item-inner">
              <div class="item-title item-label"><label for="f-name">Name</label></div>
              <div class="item-input-wrap">
                <input id="f-name" v-model="form.name" name="name" type="text" placeholder="Desktop" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false" />
              </div>
            </div>
          </li>
          <li :class="rowCls('mac')">
            <div class="item-inner">
              <div class="item-title item-label"><label for="f-mac">MAC</label></div>
              <div class="item-input-wrap">
                <input id="f-mac" v-model="form.mac" name="mac" type="text" placeholder="AA:BB:CC:DD:EE:FF" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false" />
              </div>
            </div>
          </li>
          <li :class="rowCls('ip')">
            <div class="item-inner">
              <div class="item-title item-label"><label for="f-ip">IP</label></div>
              <div class="item-input-wrap">
                <input id="f-ip" v-model="form.ip" name="ip" type="text" inputmode="url" placeholder="192.168.1.20" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false" @input="dots('ip')" />
              </div>
            </div>
          </li>
          <li :class="rowCls('broadcast')">
            <div class="item-inner">
              <div class="item-title item-label"><label for="f-broadcast">Broadcast</label></div>
              <div class="item-input-wrap">
                <input id="f-broadcast" v-model="form.broadcast" name="broadcast" type="text" inputmode="url" :placeholder="bcastPh" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false" @input="dots('broadcast')" />
              </div>
            </div>
          </li>
        </ul>
      </div>
      <div class="list list-strong inset list-dividers-ios">
        <ul>
          <li class="item-content">
            <div class="item-inner">
              <div class="item-title">
                <label for="f-agent">Has zerowol agent</label>
                <div class="item-footer">Needed for Reboot and Power off</div>
              </div>
              <div class="item-after">
                <label class="toggle"><input id="f-agent" v-model="form.agent" type="checkbox" /><span class="toggle-icon" /></label>
              </div>
            </div>
          </li>
        </ul>
      </div>
      <div class="list list-strong inset list-dividers-ios accordion-list">
        <ul>
          <li ref="adv" class="accordion-item">
            <a class="item-link item-content" href="#"><div class="item-inner"><div class="item-title">Advanced</div></div></a>
            <div class="accordion-item-content">
              <div class="list" :class="{ 'zw-inline': IOS }">
                <ul>
                  <li :class="rowCls('wol_port')">
                    <div class="item-inner">
                      <div class="item-title item-label"><label for="f-wol_port">WoL port</label></div>
                      <div class="item-input-wrap"><input id="f-wol_port" v-model="form.wol_port" name="wol_port" type="number" inputmode="numeric" placeholder="9" autocomplete="off" /></div>
                    </div>
                  </li>
                  <li :class="rowCls('probe_port')">
                    <div class="item-inner">
                      <div class="item-title item-label"><label for="f-probe_port">Probe port</label></div>
                      <div class="item-input-wrap"><input id="f-probe_port" v-model="form.probe_port" name="probe_port" type="number" inputmode="numeric" placeholder="22" autocomplete="off" /></div>
                    </div>
                  </li>
                </ul>
              </div>
            </div>
          </li>
        </ul>
      </div>
      <button type="submit" hidden />
    </form>
    <div class="block errmsg">{{ err }}</div>
    <div class="block zw-actions">
      <button class="button button-large button-fill button-round" :class="{ disabled: busy }" type="button" @click="save">Save</button>
    </div>
    <div v-if="m" class="list list-strong inset list-dividers-ios">
      <ul>
        <li v-if="m.agent">
          <a class="list-button" href="#" @click.prevent="props.f7router.navigate('/machine/' + m.id + '/pair/')">Agent install command</a>
        </li>
        <li><a class="list-button color-red" href="#" @click.prevent="del">Delete Machine</a></li>
      </ul>
    </div>
  </f7-page>
</template>
