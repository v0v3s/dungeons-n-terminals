// The Data screen's tabs: the menu lists them under Data.
import type { Status } from "./api";

export type DataTab = "Import" | "Rules" | "Backups" | "Problems" | "About";

/// The tabs there are (Problems only while there are some), each with what
/// it says (`Problems (3)`).
export function dataTabs(status: Status | null): [DataTab, string][] {
  const problems = status?.problems.length ?? 0;
  const tabs: [DataTab, string][] = [
    ["Import", "Import"],
    ["Rules", "Rules"],
    ["Backups", "Backups"],
  ];
  if (problems) tabs.push(["Problems", `Problems (${problems})`]);
  tabs.push(["About", "About"]);
  return tabs;
}
