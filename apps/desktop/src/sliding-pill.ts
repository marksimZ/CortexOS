// GPU-composited sliding active background for navigation tabs, segmented controls, and selection lists.
const CONTAINER_SELECTOR =
  ".prototype-topbar .primary-navigation, .settings-navigation, .segmented-control, .knowledge-list, .sidebar-threads > .conversation-list";

function findActiveItem(container: HTMLElement): HTMLElement | null {
  if (container.classList.contains("primary-navigation")) {
    return container.querySelector<HTMLElement>("a[aria-current]");
  }
  if (container.classList.contains("settings-navigation")) {
    return container.querySelector<HTMLElement>('[aria-pressed="true"]');
  }
  if (container.classList.contains("knowledge-list")) {
    return container.querySelector<HTMLElement>('.knowledge-row[aria-current="true"]');
  }
  if (container.classList.contains("conversation-list")) {
    return container.querySelector<HTMLElement>('.conversation-item[aria-current="true"]');
  }
  return container.querySelector<HTMLElement>("label:has(input:checked)");
}

function updateContainer(container: HTMLElement) {
  const active = findActiveItem(container);
  if (!active || container.offsetParent === null) {
    container.removeAttribute("data-pill-ready");
    container.removeAttribute("data-pill-animate");
    return;
  }
  const cRect = container.getBoundingClientRect();
  const aRect = active.getBoundingClientRect();
  if (cRect.width === 0 || aRect.width === 0) return;

  const x = aRect.left - cRect.left + container.scrollLeft;
  const y = aRect.top - cRect.top + container.scrollTop;
  const wasReady = container.getAttribute("data-pill-ready") === "true";

  container.style.setProperty("--pill-x", `${x}px`);
  container.style.setProperty("--pill-y", `${y}px`);
  container.style.setProperty("--pill-w", `${aRect.width}px`);
  container.style.setProperty("--pill-h", `${aRect.height}px`);
  container.setAttribute("data-pill-ready", "true");

  if (!wasReady) {
    requestAnimationFrame(() => {
      if (container.isConnected && container.getAttribute("data-pill-ready") === "true") {
        container.setAttribute("data-pill-animate", "true");
      }
    });
  }
}

let installed = false;

export function installSlidingPills() {
  if (installed || typeof document === "undefined") return;
  installed = true;

  let raf = 0;
  const scheduleSync = () => {
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      document.querySelectorAll<HTMLElement>(CONTAINER_SELECTOR).forEach(updateContainer);
    });
  };

  const resizeObserver = new ResizeObserver(scheduleSync);
  const observed = new WeakSet<HTMLElement>();
  const syncObservers = () => {
    document.querySelectorAll<HTMLElement>(CONTAINER_SELECTOR).forEach(el => {
      if (!observed.has(el)) {
        observed.add(el);
        resizeObserver.observe(el);
      }
      updateContainer(el);
    });
  };

  const mutationObserver = new MutationObserver(() => {
    syncObservers();
    scheduleSync();
  });

  mutationObserver.observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["aria-current", "aria-pressed", "checked", "hidden", "class"],
  });

  document.addEventListener("change", scheduleSync, true);
  window.addEventListener("hashchange", scheduleSync);
  window.addEventListener("resize", scheduleSync);
  syncObservers();
}
