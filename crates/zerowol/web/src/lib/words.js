export const HINT = " Check journalctl -u zerowol-agent on the PC.";
export const ACTION = { wake: "Wake", reboot: "Reboot", poweroff: "Power off" };
export const BUSY = { wake: "Waking…", reboot: "Rebooting…", poweroff: "Powering off…" };
export const FAIL = {
  wake: "Didn't come online",
  reboot: "Didn't reboot." + HINT,
  poweroff: "Didn't power off." + HINT,
};
export const LIMIT = { wake: 120e3, reboot: 180e3, poweroff: 90e3 };
export const REPEAT = { once: "Once", daily: "Daily", weekdays: "Weekdays", weekends: "Weekends", weekly: "Weekly" };
export const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
export const DAY_LABEL = { mon: "Mon", tue: "Tue", wed: "Wed", thu: "Thu", fri: "Fri", sat: "Sat", sun: "Sun" };
export const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
