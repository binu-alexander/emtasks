// Print the main window id of a process: its "emTasks" window, preferring one that's on screen,
// otherwise the largest. Searches every Space, so it works even when another desktop is showing.
import CoreGraphics
let pid = Int32(CommandLine.arguments[1])!
let all = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
let mine = all.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
func area(_ w: [String: Any]) -> Double {
    let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
    return (b["Width"] as? Double ?? 0) * (b["Height"] as? Double ?? 0)
}
func score(_ w: [String: Any]) -> (Int, Int, Double) {
    ((w[kCGWindowName as String] as? String) == "emTasks" ? 1 : 0,
     (w[kCGWindowIsOnscreen as String] as? Bool) == true ? 1 : 0,
     area(w))
}
if let best = mine.max(by: { score($0) < score($1) }), area(best) > 10_000 {
    print(best[kCGWindowNumber as String]!)
}
