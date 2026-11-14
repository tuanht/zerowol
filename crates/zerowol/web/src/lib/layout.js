import { ref } from "vue";
import { theme } from "framework7-vue";

// iOS vs Material. A live binding: Framework7-Vue decides the theme when main.js installs it,
// after every import has been evaluated, so initLayout() sets it before the app mounts.
export let IOS = false;
// "Wide" (tablet/desktop) switches the navigation layout live, e.g. on rotation.
export const wide = ref(false);

export function initLayout() {
  IOS = !!theme.ios;
  const q = matchMedia("(min-width: 768px)");
  wide.value = q.matches;
  q.addEventListener("change", () => (wide.value = q.matches));
}
