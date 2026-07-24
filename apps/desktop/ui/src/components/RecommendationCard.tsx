import { useState } from "react";
import { tauriBridge, type EventSummary, type Recommendation } from "../tauriBridge";

const CONFIDENCE_DOTS = 5;

function confidenceLabel(confidence: number): string {
  const filled = Math.round(confidence * CONFIDENCE_DOTS);
  return "●".repeat(filled) + "○".repeat(CONFIDENCE_DOTS - filled);
}

/**
 * docs/ux/04-recommendations-ux.md's card + expanded "Why?" view — every
 * section below maps 1:1 to a `Recommendation` field, per that doc's closing
 * note: the UI cannot render a recommendation missing any of these fields,
 * which is how FR-10's "every recommendation must include" requirement is
 * enforced rather than merely documented.
 */
export function RecommendationCard({
  recommendation,
  onStatusChange,
}: {
  recommendation: Recommendation;
  onStatusChange?: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The list endpoint doesn't carry the contributing events (FR-13's "what
  // observations contributed?"); they're fetched lazily via
  // get_recommendation_detail the first time the card is expanded, so the
  // real evidence trail is shown rather than just the recommendation's own
  // prose about itself.
  const [contributingEvents, setContributingEvents] = useState<EventSummary[] | null>(null);

  const toggleExpanded = async () => {
    const nextExpanded = !expanded;
    setExpanded(nextExpanded);
    if (nextExpanded && contributingEvents === null && recommendation.id != null) {
      try {
        const detail = await tauriBridge.getRecommendationDetail(recommendation.id);
        setContributingEvents(detail.contributing_events);
        setError(null);
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const markImplemented = async () => {
    try {
      await tauriBridge.setRecommendationStatus(recommendation.id!, "implemented");
      setError(null);
      onStatusChange?.();
    } catch (e) {
      setError(String(e));
    }
  };

  const dismiss = async (reason: string) => {
    try {
      await tauriBridge.setRecommendationStatus(recommendation.id!, "dismissed", reason);
      setError(null);
      onStatusChange?.();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <article className="recommendation-card" aria-label={recommendation.title}>
      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}
      <h3>{recommendation.title}</h3>
      <p className="recommendation-meta">
        Estimated time saved:{" "}
        <strong>{recommendation.estimated_time_saved_minutes.toFixed(0)} minutes</strong>
      </p>
      <p className="recommendation-meta">
        Recommended: {recommendation.category} · Confidence:{" "}
        <span className="confidence-dots" aria-label={`confidence ${recommendation.confidence}`}>
          {confidenceLabel(recommendation.confidence)}
        </span>{" "}
        · Difficulty: {recommendation.difficulty}
      </p>

      <div className="btn-row">
        <button className="btn" type="button" onClick={toggleExpanded}>
          {expanded ? "Hide details" : "Why?"}
        </button>
      </div>

      {expanded && (
        <div className="recommendation-detail" data-testid="recommendation-detail">
          <h4>Why this recommendation</h4>
          <p>{recommendation.why}</p>

          <h4>What we actually observed</h4>
          {contributingEvents === null ? (
            <p>Loading the observations behind this…</p>
          ) : contributingEvents.length === 0 ? (
            <p>No individual observations are still on record for this pattern.</p>
          ) : (
            <ul className="contributing-events" data-testid="contributing-events">
              {contributingEvents.map((event) => (
                <li key={event.id ?? `${event.source_id}-${event.occurred_at}`}>
                  <time>{event.occurred_at}</time> {event.source_id} — {event.signal_type}
                </li>
              ))}
            </ul>
          )}

          <h4>Assumptions made</h4>
          <ul>
            {recommendation.assumptions.map((a) => (
              <li key={a}>{a}</li>
            ))}
          </ul>

          <h4>What we ignored</h4>
          <ul>
            {recommendation.ignored_information.map((i) => (
              <li key={i}>{i}</li>
            ))}
          </ul>

          <h4>Alternatives considered</h4>
          <table>
            <thead>
              <tr>
                <th scope="col">Approach</th>
                <th scope="col">Tradeoff</th>
              </tr>
            </thead>
            <tbody>
              {recommendation.alternatives.map((alt) => (
                <tr key={alt.approach}>
                  <td>{alt.approach}</td>
                  <td>{alt.tradeoff}</td>
                </tr>
              ))}
            </tbody>
          </table>

          <p>Privacy implications: {recommendation.privacy_implications}</p>
          <p>Implementation effort: {recommendation.implementation_effort}</p>
        </div>
      )}

      <div className="btn-row">
        <button className="btn btn-primary" type="button" onClick={markImplemented}>
          Mark implemented
        </button>
        <button className="btn" type="button" onClick={() => dismiss("not worth the effort")}>
          Dismiss
        </button>
      </div>
    </article>
  );
}
