import { useCallback, useEffect, useRef, useState } from "react";
import {
  tauriBridge,
  type EventSummary,
  type PrivacyManifestStatus,
  type PrivacyState,
} from "../tauriBridge";

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
  const [manifestStatus, setManifestStatus] = useState<PrivacyManifestStatus | null>(null);
  const [events, setEvents] = useState<EventSummary[]>([]);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Focus management for the delete-all dialog (docs/ux/06-accessibility.md
  // §1): the Cancel button is focused on open (never Delete — a stray Enter
  // must not destroy data), and focus is restored to the button that opened
  // the dialog when it closes.
  const deleteTriggerRef = useRef<HTMLButtonElement>(null);
  const cancelDeleteRef = useRef<HTMLButtonElement>(null);

  const openDeleteConfirm = () => setConfirmingDelete(true);
  const closeDeleteConfirm = useCallback(() => {
    setConfirmingDelete(false);
    deleteTriggerRef.current?.focus();
  }, []);

  useEffect(() => {
    if (confirmingDelete) {
      cancelDeleteRef.current?.focus();
    }
  }, [confirmingDelete]);

  const refresh = useCallback(async () => {
    try {
      const [nextStatus, nextManifestStatus, nextEvents] = await Promise.all([
        tauriBridge.getObservationStatus(),
        tauriBridge.getPrivacyManifestStatus(),
        tauriBridge.getRecentEvents(20),
      ]);
      setStatus(nextStatus);
      setManifestStatus(nextManifestStatus);
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
    try {
      if (status.observation_active) {
        await tauriBridge.pauseObservation();
      } else {
        await tauriBridge.resumeObservation();
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const confirmDeleteAll = async () => {
    try {
      await tauriBridge.deleteAllData();
      closeDeleteConfirm();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const acknowledgeManifest = async () => {
    try {
      await tauriBridge.acknowledgePrivacyManifest();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <section aria-label="Privacy Dashboard">
      <h1>Privacy Dashboard</h1>

      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      {manifestStatus?.reconsent_required && (
        <div className="alert card" role="alert" data-testid="reconsent-banner">
          <p>
            What HiddenSteps captures at your privacy level has changed since
            you last agreed to it. Observation is paused until you review and
            accept the update.
          </p>
          <button className="btn" type="button" onClick={acknowledgeManifest}>
            Review and continue observing
          </button>
        </div>
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
          // aria-live="polite" per docs/ux/06-accessibility.md §1: new captured
          // rows are ambient reference information, announced without seizing a
          // screen-reader user's focus or interrupting their current task.
          <ul className="event-list" data-testid="recent-events" aria-live="polite">
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
          <button
            ref={deleteTriggerRef}
            className="btn btn-danger"
            type="button"
            onClick={openDeleteConfirm}
          >
            Delete all data
          </button>
        ) : (
          <div
            className="confirm-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-label="Delete all HiddenSteps data?"
            // Esc cancels this destructive dialog (docs/ux/06-accessibility.md
            // §2). There is deliberately no Enter-to-confirm shortcut — the
            // "Delete everything" control must be explicitly activated.
            onKeyDown={(e) => {
              if (e.key === "Escape") closeDeleteConfirm();
            }}
          >
            <p>
              This removes every captured summary, pattern, recommendation, and
              setting — permanently. This cannot be undone.
            </p>
            <p>
              Your encryption key will also be deleted, so even a backup copy of
              this data becomes unreadable.
            </p>
            <div className="btn-row">
              <button
                ref={cancelDeleteRef}
                className="btn"
                type="button"
                onClick={closeDeleteConfirm}
              >
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
