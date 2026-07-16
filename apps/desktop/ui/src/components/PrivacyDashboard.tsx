import { useCallback, useEffect, useState } from "react";
import { tauriBridge, type EventSummary, type PrivacyState } from "../tauriBridge";

const LEVEL_LABELS: Record<number, string> = {
  0: "Manual",
  1: "App awareness",
  2: "Workflow awareness",
  3: "Context-aware",
  4: "Maximum assistance",
};

/**
 * The persistent trust surface — docs/ux/03-privacy-dashboard.md. The recent-
 * events feed renders exactly what `get_recent_events` returns; there is no
 * client-side transformation between what's fetched and what's shown, per the
 * trust-model claim in docs/design/04-trust-model.md §2 that this feed must show
 * exactly what's stored.
 */
export function PrivacyDashboard() {
  const [status, setStatus] = useState<PrivacyState | null>(null);
  const [events, setEvents] = useState<EventSummary[]>([]);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatus, nextEvents] = await Promise.all([
        tauriBridge.getObservationStatus(),
        tauriBridge.getRecentEvents(20),
      ]);
      setStatus(nextStatus);
      setEvents(nextEvents);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const togglePause = async () => {
    if (!status) return;
    if (status.observation_active) {
      await tauriBridge.pauseObservation();
    } else {
      await tauriBridge.resumeObservation();
    }
    await refresh();
  };

  const confirmDeleteAll = async () => {
    await tauriBridge.deleteAllData();
    setConfirmingDelete(false);
    await refresh();
  };

  return (
    <section aria-label="Privacy Dashboard">
      <h1>Privacy Dashboard</h1>

      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      {status && (
        <p className="status-line card" data-testid="status-line">
          <span
            className={`status-indicator ${status.observation_active ? "is-active" : "is-paused"}`}
            data-testid="status-indicator"
          >
            {status.observation_active ? "● Observing" : "○ Paused"}
          </span>
          {" — "}
          {LEVEL_LABELS[status.current_level] ?? `Level ${status.current_level}`}
          <button className="btn" type="button" onClick={togglePause}>
            {status.observation_active ? "Pause" : "Resume"}
          </button>
        </p>
      )}

      <div className="section-block">
        <h2>What's being captured right now</h2>
        {events.length === 0 ? (
          <p>Nothing captured yet.</p>
        ) : (
          <ul className="event-list" data-testid="recent-events">
            {events.map((event) => (
              <li key={event.id ?? `${event.source_id}-${event.occurred_at}`}>
                <time>{event.occurred_at}</time> {event.source_id} — {event.signal_type}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="section-block">
        {!confirmingDelete ? (
          <button className="btn btn-danger" type="button" onClick={() => setConfirmingDelete(true)}>
            Delete all data
          </button>
        ) : (
          <div className="confirm-dialog" role="alertdialog" aria-label="Delete all HiddenSteps data?">
            <p>
              This removes every captured summary, pattern, recommendation, and
              setting — permanently. This cannot be undone.
            </p>
            <p>
              Your encryption key will also be deleted, so even a backup copy of
              this data becomes unreadable.
            </p>
            <div className="btn-row">
              <button className="btn" type="button" onClick={() => setConfirmingDelete(false)}>
                Cancel
              </button>
              <button className="btn btn-danger" type="button" onClick={confirmDeleteAll}>
                Delete everything
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
