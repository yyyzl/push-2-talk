/** Keep initialization separate from the debounce/synchronization window. */
export function createConfigInitialization() {
  let started = false;
  let loaded = false;
  return {
    isStarted: () => started,
    isReady: () => loaded,
    async run(load: () => Promise<void>): Promise<void> {
      if (started) return;
      started = true;
      try {
        await load();
        loaded = true;
      } catch (error) {
        started = false;
        loaded = false;
        throw error;
      }
    },
  };
}
