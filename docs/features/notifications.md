# Notifications

Chibby records what finished, so you can step away from a run and still find out how it went.

## The inbox

The bell in the sidebar opens the inbox and shows the unread count. The count is red when an unread entry is a failure.

An entry is recorded for:

- Every finished run: manual, release, retry, rollback, scheduled, file-watch, and CLI or git hook runs
- A run that could not start, such as a busy repo or failed preflight
- A run that was interrupted because Chibby stopped mid-run, reported on next launch

A failed automatic rollback appears once, on the run that triggered it, with the rollback's outcome in its text.

Opening a run marks its entries read, and **Mark all read** clears the rest. Chibby keeps the newest 500 entries and removes read entries after 30 days. An entry can outlive its run's logs; opening one then shows that the run is no longer available.

Consecutive unread successes from the same schedule or file watch collapse into the latest one, so a busy watch cannot bury a failure.

## Alerts and webhooks

The inbox always records. `.chibby/notify.toml` and the **Default Notifications** settings only decide which OS alerts and webhooks are sent:

| Config                                                    | Effect                                                     |
| --------------------------------------------------------- | ---------------------------------------------------------- |
| Project has `.chibby/notify.toml`                         | Used as written                                            |
| No `notify.toml`                                          | App defaults from **Settings** send desktop alerts         |
| `notify.toml` does not parse                              | Desktop alert is still sent; no webhooks                   |
| Schedule or watch fails, `always_on_failure` on (default) | Alert and all webhooks, even if notifications are disabled |

Toasts appear only while the Chibby window is in front.

## Storage

Each entry is one JSON file in `<data_dir>/notifications/`. The app and the CLI are separate processes, so one file per entry keeps their writes independent. The app watches this folder, which is how CLI runs reach the inbox while it is open.
