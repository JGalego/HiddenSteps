import { invoke } from "@tauri-apps/api/core";

// Typed wrapper over the commands in ../../../../docs/design/09-api-specification.md
// §2. Every UI component calls through here rather than `invoke` directly, so a
// test can mock this one module instead of reaching into `@tauri-apps/api`.

export interface PrivacyState {
  current_level: number;
  consented_manifest_version: number;
  observation_active: boolean;
  updated_at: string;
}

export interface EventSummary {
  id: number | null;
  occurred_at: string;
  source_id: string;
  signal_type: string;
  privacy_level_at_capture: number;
  summary: Record<string, unknown>;
  is_deep_mode: boolean;
  ttl_expires_at: string | null;
}

export interface Alternative {
  approach: string;
  tradeoff: string;
}

export interface Recommendation {
  id: number | null;
  pattern_id: number;
  created_at: string;
  title: string;
  category: string;
  why: string;
  confidence: number;
  estimated_time_saved_minutes: number;
  difficulty: "low" | "medium" | "high";
  maintenance_burden: "low" | "medium" | "high";
  privacy_implications: string;
  implementation_effort: string;
  alternatives: Alternative[];
  assumptions: string[];
  ignored_information: string[];
  generating_provider: string;
  status: "suggested" | "implemented" | "dismissed";
  dismissal_reason: string | null;
}

export const tauriBridge = {
  getObservationStatus: (): Promise<PrivacyState> => invoke("get_observation_status"),

  pauseObservation: (): Promise<boolean> => invoke("pause_observation"),

  resumeObservation: (): Promise<boolean> => invoke("resume_observation"),

  getRecentEvents: (limit: number): Promise<EventSummary[]> =>
    invoke("get_recent_events", { limit }),

  deleteEvents: (eventIds: number[]): Promise<number> =>
    invoke("delete_events", { eventIds }),

  exportData: (): Promise<Record<string, unknown>> => invoke("export_data"),

  deleteAllData: (): Promise<boolean> => invoke("delete_all_data"),

  listRecommendations: (statusFilter?: string): Promise<Recommendation[]> =>
    invoke("list_recommendations", { statusFilter }),

  setRecommendationStatus: (
    id: number,
    status: "implemented" | "dismissed",
    dismissalReason?: string
  ): Promise<boolean> =>
    invoke("set_recommendation_status", {
      request: { id, status, dismissal_reason: dismissalReason },
    }),
};
