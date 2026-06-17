// Remembers the last active sub-tab for pages that have internal tabs (Library,
// Settings) so returning to the page via the sidebar restores the tab the user
// last viewed instead of resetting to the first one. Module scope = survives
// component unmount/remount for the app's lifetime.
const tabStore = new Map<string, string>();

export function getLastTab(page: string): string | undefined {
  return tabStore.get(page);
}

export function setLastTab(page: string, tab: string): void {
  tabStore.set(page, tab);
}
