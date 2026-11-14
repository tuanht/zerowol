import { f7 } from "framework7-vue";
import { esc, offlineMsg } from "./api.js";
import { IOS } from "./layout.js";
import { confirmDialog, toast } from "./ui.js";
import { ACTION } from "./words.js";
import { deleteSchedule, openScheduleForm, reloadSchedules, schedState, skip, unskip } from "./schedules.js";

// Runs an API call; on success bumps schedState.rev so the Schedules page reloads (rules and occurrences).
async function run(call) {
  try {
    await call();
  } catch (e) {
    toast(offlineMsg(e));
    return false;
  }
  schedState.rev++;
  return true;
}

async function editRule(id) {
  let s = schedState.list.find((x) => x.id === id);
  if (!s) {
    try {
      await reloadSchedules();
    } catch (e) {
      toast(offlineMsg(e));
      return false;
    }
    s = schedState.list.find((x) => x.id === id);
  }
  if (!s) {
    toast("Schedule not found");
    return false;
  }
  return openScheduleForm({ schedule: s });
}

// Past / skipped status of an occurrence for its agenda row ("" for an upcoming one).
export function outcomeText(o, now) {
  if (o.skipped) return "skipped";
  if (o.at > now || !o.outcome) return "";
  const { result, detail } = o.outcome;
  if (result === "ok") return detail === "already online" ? "done · already online" : "done";
  if (result === "failed") return "failed: " + (detail || "error");
  if (result === "missed") return "missed";
  return "unknown"; // "firing": the action started but its result was never logged
}

// Tapping an occurrence: a once rule opens its form; a repeating one gets an action sheet
// (iOS) / bottom sheet (Material). Resolves true when something changed.
export function openOccurrenceActions(occ, targetEl) {
  if (occ.repeat_kind === "once") return editRule(occ.schedule_id);
  return new Promise((resolve) => {
    // The picked action runs once the sheet has closed, so its confirm dialog or form isn't stacked on it.
    let picked = null;
    const pick = (fn) => () => (picked = fn);
    const groups = [
      [
        { text: esc(occ.machine_name + " · " + ACTION[occ.action] + " · " + occ.time), label: true },
        occ.skipped
          ? { text: "Don't skip", onClick: pick(() => run(() => unskip(occ.schedule_id, occ.date))) }
          : { text: "Skip this time", onClick: pick(() => run(() => skip(occ.schedule_id, occ.date))) },
        { text: "Edit rule…", onClick: pick(() => editRule(occ.schedule_id)) },
        {
          text: "Delete rule",
          color: "red",
          onClick: pick(async () => {
            const ok = await confirmDialog({ title: "Delete this schedule?", text: "", label: "Delete", red: true });
            return ok && run(() => deleteSchedule(occ.schedule_id));
          }),
        },
      ],
    ];
    if (IOS) groups.push([{ text: "Cancel", strong: true }]);
    const sheet = f7.actions.create({
      buttons: groups,
      // A popover beside the row on wide screens, once the Popover component is registered (Task 7).
      convertToPopover: !!f7.popover,
      targetEl,
      destroyOnClose: true,
      on: { closed: () => resolve(picked ? picked() : false) },
    });
    sheet.open();
  });
}
