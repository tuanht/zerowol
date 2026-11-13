// Android system back for tabbed views.
//
// Framework7's browserHistory keeps one linear history for all views, so a
// back press on one tab can pop a page off another, hidden tab. Instead, while
// the active tab has a sub-page we keep exactly one extra same-URL history
// entry (the guard). The system back button pops the guard, and we pop the
// active tab's own page stack. On a tab root there is no guard, so back leaves
// the app as usual, but never from a sub-page.
export function installBackGuard(f7, activeView) {
  const isGuard = (s) => !!(s && s.zwGuard);
  let armed = isGuard(history.state);
  let skip = 0; // popstate events caused by our own history.back()

  const deep = () => {
    const v = activeView();
    return !!(v && v.router && v.router.history.length > 1);
  };
  const arm = () => {
    history.pushState({ zwGuard: 1 }, "");
    armed = true;
  };
  const sync = () => {
    const want = deep();
    if (want && !armed) arm();
    else if (!want && armed && isGuard(history.state)) {
      armed = false;
      skip++;
      history.back();
    }
  };

  addEventListener("popstate", (e) => {
    if (skip) {
      skip--;
      return;
    }
    armed = isGuard(e.state);
    if (armed) return; // forward navigation onto a guard
    const v = activeView();
    if (!v || v.router.history.length < 2) return;
    const depth = v.router.history.length;
    const moved = v.router.allowPageChange;
    if (moved) v.router.back();
    // Re-arm at once when a sub-page remains, so a quick second press during
    // the transition still stays inside the app.
    if (!moved || depth > 2) arm();
  });
  f7.on("routeChanged", sync);
  f7.on("tabShow", sync);
  sync();
}
