import { createApp } from "vue";
import Framework7 from "framework7/lite";
import Dialog from "framework7/components/dialog";
import Toast from "framework7/components/toast";
import Toggle from "framework7/components/toggle";
import Input from "framework7/components/input";
import Fab from "framework7/components/fab";
import Preloader from "framework7/components/preloader";
import Accordion from "framework7/components/accordion";
import Card from "framework7/components/card";
import Popup from "framework7/components/popup";
import Sheet from "framework7/components/sheet";
import Tabs from "framework7/components/tabs";
import Actions from "framework7/components/actions";
import Framework7Vue, {
  f7App, f7BlockFooter, f7Link, f7Navbar, f7NavRight, f7Page, f7Popup, f7Sheet, f7Toolbar, f7ToolbarPane, f7View, f7Views,
} from "framework7-vue";

import "framework7/css";
import "framework7/components/dialog/css";
import "framework7/components/toast/css";
import "framework7/components/toggle/css";
import "framework7/components/input/css";
import "framework7/components/fab/css";
import "framework7/components/preloader/css";
import "framework7/components/accordion/css";
import "framework7/components/card/css";
import "framework7/components/radio/css";
import "framework7/components/popup/css";
import "framework7/components/sheet/css";
import "framework7/components/tabs/css";
import "framework7/components/actions/css";
import "framework7/components/typography/css";
import "./styles/app.css";

import App from "./App.vue";
import { themePref, darkParam } from "./lib/prefs.js";
import { initLayout } from "./lib/layout.js";

const theme = themePref();
Framework7.use([Dialog, Toast, Toggle, Input, Fab, Preloader, Accordion, Card, Popup, Sheet, Tabs, Actions]);
Framework7.use(Framework7Vue, { theme });
initLayout();

// Old installs may carry a "#!/..." hash from the previous router; start clean.
if (location.hash) history.replaceState(null, "", location.pathname + location.search);
// navigator.standalone exists only in iOS/iPadOS home-screen apps: the OS blurs a band below the
// status bar there whichever theme is picked, so this keys on the device, not on Framework7's theme class.
if (navigator.standalone) document.documentElement.classList.add("zw-ios-pwa");

const f7params = {
  name: "zerowol",
  theme,
  darkMode: darkParam(),
  colors: { primary: "#007aff" },
  touch: { tapHold: false },
  // Browser history is handled by lib/back.js (per-tab Android back), not by
  // Framework7, whose single history stack mixes the tabs' page stacks.
  view: { browserHistory: false, iosSwipeBack: true },
};

const app = createApp(App, { f7params });
// Register only the components the templates use (the "bundle" entry pulls in all of them).
const components = {
  f7App, f7BlockFooter, f7Link, f7Navbar, f7NavRight, f7Page, f7Popup, f7Sheet, f7Toolbar, f7ToolbarPane, f7View, f7Views,
};
for (const [name, component] of Object.entries(components)) {
  app.component(name.replace(/([A-Z])/g, "-$1").toLowerCase(), component);
}
app.mount("#app");

if (import.meta.env.PROD && "serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch(() => {});
}
