// Slow work in progress (switching campaigns, importing, …): the page
// shows what it is and takes no taps until it is done, so nothing is
// started twice.

let tasks = $state<string[]>([]);

export const busy = {
  /// What is being done, if anything.
  get label(): string | null {
    return tasks.at(-1) ?? null;
  },
};

/// Run `work`, showing `label` meanwhile.
export async function working<T>(label: string, work: () => Promise<T>): Promise<T> {
  tasks = [...tasks, label];
  try {
    return await work();
  } finally {
    const i = tasks.lastIndexOf(label);
    tasks = tasks.filter((_, j) => j !== i);
  }
}
