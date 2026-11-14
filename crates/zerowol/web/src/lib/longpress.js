// Long press (and double-click, its mouse equivalent) on an empty calendar slot creates a one-off schedule there.
// The pure parts work out the prefilled time; longPress() is the shared pointer logic of the Week and Month grids.

const pad = (n) => String(n).padStart(2, "0");
const hhmm = (m) => pad(Math.floor(m / 60)) + ":" + pad(m % 60);
// The first quarter hour strictly after `min` (16:42 → 16:45, 16:45 → 17:00); null past 23:45.
const nextQuarter = (min) => {
  const q = (Math.floor(min / 15) + 1) * 15;
  return q < 1440 ? hhmm(q) : null;
};

// Week slot (date, hour) → "HH:MM", or null when the slot is over. `clock` is now in the schedule timezone:
// { date: "YYYY-MM-DD", min: minutes since midnight }. The current hour gets its next quarter hour
// (or the next hour's HH:00 once none is left).
export function slotTime(date, hour, clock) {
  if (date < clock.date || (date === clock.date && hour < Math.floor(clock.min / 60))) return null;
  if (date > clock.date || hour > Math.floor(clock.min / 60)) return hhmm(hour * 60);
  return nextQuarter(clock.min);
}

// Month day → "08:00", or for today once 08:00 has passed the next quarter hour; null for days that are over.
export function dayTime(date, clock) {
  if (date < clock.date) return null;
  if (date > clock.date || clock.min < 480) return "08:00";
  return nextQuarter(clock.min);
}

// Pointer handlers for a grid (bind them on the grid element; call destroy() when it unmounts).
// `find(target)` returns the slot element under the pointer (or null: no long press there);
// `resolve(el)` returns what to run for it, or null when the slot is over (no feedback then).
export function longPress({ find, resolve, delay = 500, slop = 10 }) {
  let timer = 0;
  let id = null; // the pointer being held (a second one is ignored)
  let x = 0;
  let y = 0;
  let pressed = null;
  let unpressTimer = 0;
  let disarmTimer = 0;
  let held = false; // a press is pending or has just fired: its context menu is ours to suppress
  // The click that follows a long press (with a mouse it lands on whatever the opened popup / popover left under
  // the pointer, or on a common ancestor, which F7 takes as an outside click closing a popover) is swallowed.
  // Touch browsers may send none, so the next press or key anywhere, or 1 s passing, drops the swallower.
  const swallow = (e) => {
    e.stopPropagation();
    e.preventDefault();
    disarm();
  };
  const disarm = () => {
    clearTimeout(disarmTimer);
    document.removeEventListener("click", swallow, true);
    document.removeEventListener("pointerdown", disarm, true);
    document.removeEventListener("keydown", disarm, true);
  };
  const arm = () => {
    disarm();
    document.addEventListener("click", swallow, true);
    document.addEventListener("pointerdown", disarm, true);
    document.addEventListener("keydown", disarm, true);
    disarmTimer = setTimeout(disarm, 1000);
  };
  const clear = () => {
    clearTimeout(timer);
    timer = 0;
    id = null;
  };
  const unpress = () => {
    clearTimeout(unpressTimer);
    if (pressed) pressed.classList.remove("zw-pressed");
    pressed = null;
  };
  // Runs the slot's action with feedback: a pressed highlight and a short vibration where supported.
  function trigger(el, long) {
    const run = el && resolve(el);
    if (!run) return;
    if (long) arm(); // the click that follows the long press is not a tap
    unpress();
    pressed = el;
    el.classList.add("zw-pressed");
    unpressTimer = setTimeout(unpress, 600);
    navigator.vibrate?.(10);
    run();
  }
  return {
    trigger,
    pointerdown(e) {
      if (timer) return; // another pointer is already held
      held = false;
      if (e.button) return; // primary button / touch / pen only
      const el = find(e.target);
      if (!el) return;
      id = e.pointerId;
      held = e.pointerType !== "mouse";
      x = e.clientX;
      y = e.clientY;
      timer = setTimeout(() => {
        timer = 0;
        id = null;
        trigger(el, true);
      }, delay);
    },
    // Moving (e.g. the start of a scroll) cancels it.
    pointermove(e) {
      if (timer && e.pointerId === id && Math.hypot(e.clientX - x, e.clientY - y) > slop) clear();
    },
    end(e) {
      if (e && id !== null && e.pointerId !== id) return;
      clear();
      unpress();
    },
    dblclick(e) {
      trigger(find(e.target), false);
    },
    // Touch / pen long presses open no context menu over the grid; a mouse right-click keeps the browser's.
    contextmenu(e) {
      if (held) e.preventDefault();
    },
    destroy() {
      clear();
      unpress();
      disarm();
    },
  };
}
