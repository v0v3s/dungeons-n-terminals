// Open dialogs, newest last, so Android's back closes the top one first.
const open: (() => void)[] = [];

export function opened(close: () => void): () => void {
  open.push(close);
  return () => {
    const i = open.lastIndexOf(close);
    if (i >= 0) open.splice(i, 1);
  };
}

/// Close the top dialog; false when none is open.
export function closeTop(): boolean {
  const close = open.at(-1);
  close?.();
  return !!close;
}
