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

// Pointer handlers for a grid (bind them on the grid element).
// `find(target)` returns the slot element under the pointer (or null: no long press there);
// `resolve(el)` returns what to run for it, or null when the slot is over (no feedback then).
export function longPress({ find, resolve, delay = 500, slop = 10 }) {
  let timer = 0;
  let x = 0;
  let y = 0;
  let pressed = null;
  // The click that follows a long press (with a mouse it lands on whatever the opened popup / popover left under
  // the pointer, or on a common ancestor, which F7 takes as an outside click closing a popover) is swallowed;
  // touch browsers may send none, so the next press anywhere drops the swallower.
  const swallow = (e) => {
    e.stopPropagation();
    e.preventDefault();
    disarm();
  };
  const disarm = () => {
    document.removeEventListener("click", swallow, true);
    document.removeEventListener("pointerdown", disarm, true);
  };
  const arm = () => {
    document.addEventListener("click", swallow, true);
    document.addEventListener("pointerdown", disarm, true);
  };
  const clear = () => {
    clearTimeout(timer);
    timer = 0;
  };
  const unpress = () => {
    if (pressed) pressed.classList.remove("zw-pressed");
    pressed = null;
  };
  // Runs the slot's action with feedback: a pressed highlight and a short vibration where supported.
  function trigger(el) {
    const run = el && resolve(el);
    if (!run) return;
    unpress();
    pressed = el;
    el.classList.add("zw-pressed");
    setTimeout(unpress, 600);
    navigator.vibrate?.(10);
    run();
  }
  return {
    trigger,
    pointerdown(e) {
      clear();
      if (e.button) return; // primary button / touch / pen only
      const el = find(e.target);
      if (!el) return;
      x = e.clientX;
      y = e.clientY;
      timer = setTimeout(() => {
        timer = 0;
        arm(); // the click that follows the long press is not a tap
        trigger(el);
      }, delay);
    },
    // Moving (e.g. the start of a scroll) cancels it.
    pointermove(e) {
      if (timer && Math.hypot(e.clientX - x, e.clientY - y) > slop) clear();
    },
    end() {
      clear();
      unpress();
    },
    dblclick(e) {
      trigger(find(e.target));
    },
    contextmenu(e) {
      e.preventDefault();
    },
  };
}
