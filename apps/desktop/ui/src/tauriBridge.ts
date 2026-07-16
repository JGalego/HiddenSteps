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

export interface OnboardingState {
  completed: boolean;
}

export interface DetectedRuntime {
  name: string;
  reachable: boolean;
  models: string[];
}

export interface TestProviderConnectivityResponse {
  ok: boolean;
  error: string | null;
}

export interface LlmProviderConfig {
  id: string;
  provider_type: string;
  is_local: boolean;
  model_name: string | null;
  endpoint: string | null;
  vault_key_ref: string | null;
  active: boolean;
}

export interface Diagnostics {
  privacy_level: number;
  observation_active: boolean;
  active_provider: LlmProviderConfig | null;
  event_count: number;
  pattern_count: number;
  recommendation_count: number;
  audit_log_count: number;
  storage_bytes: number | null;
  encryption_status: string;
}

export interface AuditEntry {
  id: number | null;
  occurred_at: string;
  actor: "user" | "system";
  action_type: string;
  details: Record<string, unknown>;
}

export const tauriBridge = {
  getOnboardingState: (): Promise<OnboardingState> => invoke("get_onboarding_state"),

  getProviderDetection: (): Promise<DetectedRuntime[]> => invoke("get_provider_detection"),

  testProviderConnectivity: (request: {
    provider_type: string;
    endpoint?: string;
    api_key?: string;
    model?: string;
  }): Promise<TestProviderConnectivityResponse> =>
    invoke("test_provider_connectivity", { request }),

  setAiProvider: (request: {
    id: string;
    provider_type: string;
    is_local: boolean;
    model_name?: string;
    endpoint?: string;
    api_key?: string;
  }): Promise<boolean> => invoke("set_ai_provider", { request }),

  listLlmProviders: (): Promise<LlmProviderConfig[]> => invoke("list_llm_providers"),

  setPrivacyLevel: (level: number, acknowledgedPermissions: string[] = []): Promise<{ effective_level: number }> =>
    invoke("set_privacy_level", {
      request: { level, acknowledged_permissions: acknowledgedPermissions },
    }),

  completeOnboarding: (): Promise<boolean> => invoke("complete_onboarding"),

  getSettings: (key: string): Promise<unknown> => invoke("get_settings", { key }),

  updateSettings: (key: string, value: unknown): Promise<boolean> =>
    invoke("update_settings", { key, value }),

  getDiagnostics: (): Promise<Diagnostics> => invoke("get_diagnostics"),

  getAuditLog: (limit: number): Promise<AuditEntry[]> => invoke("get_audit_log", { limit }),

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
