import { useState } from "react";
import { tauriBridge, type Recommendation } from "../tauriBridge";

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

  const markImplemented = async () => {
    await tauriBridge.setRecommendationStatus(recommendation.id!, "implemented");
    onStatusChange?.();
  };

  const dismiss = async (reason: string) => {
    await tauriBridge.setRecommendationStatus(recommendation.id!, "dismissed", reason);
    onStatusChange?.();
  };

  return (
    <article aria-label={recommendation.title}>
      <h3>{recommendation.title}</h3>
      <p>
        Estimated time saved:{" "}
        <strong>{recommendation.estimated_time_saved_minutes.toFixed(0)} minutes</strong>
      </p>
      <p>
        Recommended: {recommendation.category} · Confidence:{" "}
        <span aria-label={`confidence ${recommendation.confidence}`}>
          {confidenceLabel(recommendation.confidence)}
        </span>{" "}
        · Difficulty: {recommendation.difficulty}
      </p>

      <button type="button" onClick={() => setExpanded((v) => !v)}>
        {expanded ? "Hide details" : "Why?"}
      </button>

      {expanded && (
        <div data-testid="recommendation-detail">
          <h4>Why this recommendation</h4>
          <p>{recommendation.why}</p>

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

      <button type="button" onClick={markImplemented}>
        Mark implemented
      </button>
      <button type="button" onClick={() => dismiss("not worth the effort")}>
        Dismiss
      </button>
    </article>
  );
}
