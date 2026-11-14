import { f7 } from "framework7-vue";
import { esc, offlineMsg } from "./api.js";
import { IOS, wide } from "./layout.js";
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

// Greyed when the rule is off, except an occurrence that already ran (a once rule switches itself off after firing).
export const isOff = (o, now) => !o.enabled && !(o.outcome && o.at <= now);

// Colour of an occurrence's dot in the week strip and month grid: grey ("off") when the rule is off or
// the occurrence is skipped, else its action colour.
export const dotClass = (o, now) => (o.skipped || isOff(o, now) ? "off" : o.action);

// Skip / Don't skip is offered only for an occurrence still ahead that hasn't run (a past one keeps its outcome).
export const canSkip = (o, now) => o.at > now && !o.outcome;

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
// (iOS) / bottom sheet (Material) on phones, a popover beside targetEl on wide screens.
// Resolves true when something changed.
export function openOccurrenceActions(occ, targetEl) {
  if (occ.repeat_kind === "once") return editRule(occ.schedule_id);
  return new Promise((resolve) => {
    // The picked action runs once the sheet has closed, so its confirm dialog or form isn't stacked on it.
    let picked = null;
    const pick = (fn) => () => (picked = fn);
    const groups = [
      [
        { text: esc(occ.machine_name + " · " + ACTION[occ.action] + " · " + occ.time), label: true },
        ...(canSkip(occ, Date.now() / 1000)
          ? [
              occ.skipped
                ? { text: "Don't skip", onClick: pick(() => run(() => unskip(occ.schedule_id, occ.date))) }
                : { text: "Skip this time", onClick: pick(() => run(() => skip(occ.schedule_id, occ.date))) },
            ]
          : []),
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
    // Only on wide screens: Framework7 would also convert on any desktop browser, whatever its width.
    const popover = wide.value && !!targetEl;
    // A popover closes on a tap outside it, so only the iOS action sheet gets a Cancel button.
    if (IOS && !popover) groups.push([{ text: "Cancel", strong: true }]);
    const sheet = f7.actions.create({
      buttons: groups,
      convertToPopover: popover,
      targetEl,
      destroyOnClose: true,
      on: { closed: () => resolve(picked ? picked() : false) },
    });
    sheet.open();
  });
}
