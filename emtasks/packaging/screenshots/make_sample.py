# Writes a emTasks data file with made-up sample tasks (dates relative to today) for screenshots.
# Usage: python3 make_sample.py <Theme> <output .ron> [Matrix|List] [demo step 1-4]
# Dates are fixed days relative to today (and a 48 h urgent window), so shots look the same at any time of day.
import sys, time, datetime as dt
theme, out = sys.argv[1], sys.argv[2]
view = sys.argv[3] if len(sys.argv) > 3 else "Matrix"
# Website walkthrough: one extra task shown at each step (Inbox → Schedule → moved to Do first → done).
demo = int(sys.argv[4]) if len(sys.argv) > 4 else 0
now = int(time.time())
today = int(dt.datetime.combine(dt.date.today(), dt.time()).timestamp())
D, H = 86400, 3600
tasks = []
def t(title, sec, due=None, all_day=False, tags=(), done_ago=None, notes="", repeat="None", every=1, created_ago=3, auto_from=None):
    i = len(tasks) + 1
    f = [f"id:{i}", f'title:"{title}"', f'notes:"{notes}"', f"section:{sec}",
         f"completed:{'true' if done_ago is not None else 'false'}",
         f"created_at:{now - created_ago*D}",
         f"completed_at:{'Some(%d)' % (now - done_ago) if done_ago is not None else 'None'}",
         f"due:{'Some(%d)' % due if due else 'None'}", f"all_day:{'true' if all_day else 'false'}",
         f"repeat:{repeat}", f"repeat_every:{every}",
         "tags:[" + ",".join(f'"{g}"' for g in tags) + "]", "wait_for:None", f"auto_from:{'Some(%s)' % auto_from if auto_from else 'None'}"]
    tasks.append("(" + ",".join(f) + ")")
t("Reply to Sam about the venue", "Unsorted", tags=["email"])
t("Look into standing desks", "Unsorted", tags=["home"])
t("Renew passport", "Unsorted")
t("Submit Q3 tax return", "Do", due=today - D + 17*H, tags=["finance"], notes="Receipts are in the Taxes folder.")
t("Finish client proposal", "Do", due=today + D + 11*H, tags=["work"])
if not demo:
    t("Pick up prescription", "Do", due=today, all_day=True, tags=["health"])
t("Prepare slides for Monday review", "Schedule", due=today + D + 15*H, tags=["work"])
t("Weekly planning", "Schedule", due=today + 4*D + 9*H, tags=["routine"], repeat="Week")
t("Book annual health check-up", "Schedule", due=today + 12*D, all_day=True, tags=["health"])
t("Read 'Deep Work', chapter 3", "Schedule", tags=["learning"])
t("Call bank about card fee", "Delegate", due=today + 2*D + 11*H, tags=["finance", "phone"])
t("Schedule team offsite logistics", "Delegate", tags=["work"])
t("Order printer ink", "Delegate", tags=["home"], done_ago=20*H)
t("Organise old photos", "Eliminate", tags=["home"])
t("Reorganise bookmarks", "Eliminate")
t("Draft blog post ideas", "Schedule", tags=["writing"], done_ago=2*D)
# Step 3 is saved in Schedule with a deadline tomorrow: emTasks itself moves it to Do first (⚡) on launch.
demo_task = {
    1: dict(sec="Unsorted"),
    2: dict(sec="Schedule"),
    3: dict(sec="Schedule", due=today + D + 10*H),
    4: dict(sec="Do", due=today + D + 10*H, auto_from="Schedule", done_ago=600),
}.get(demo)
if demo_task:
    t("Renew car insurance", tags=["home"], created_ago=0, **demo_task)
data = f"""(version:1,tasks:[{",".join(tasks)}],next_id:{len(tasks)+1},
filter:(tags:[],status:Active,sort:Manual),presets:[],view:{view},theme:{theme},
settings:(auto_urgent:true,urgent_hours:48,show_inbox:true,documents_backup:false,keep_completed:Week))"""
open(out, "w").write(data)
