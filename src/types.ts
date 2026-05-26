export interface SearchResult {
  path: string;
  filename: string;
  score: number;
  match_type: string;
  size_bytes: number | null;
  modified: string | null;
  file_type: string | null;
  content_snippet: string | null;
  is_dir: boolean;
  interactions: number;
}

export interface SearchResponse {
  query: string;
  mode: string;
  elapsed_ms: number;
  total_results: number;
  results: SearchResult[];
}

export type LicenseStatus =
  | "active"
  | "trial"
  | "trial_expired"
  | "invalid"
  | "unknown";

export interface LicenseState {
  status: LicenseStatus;
  key: string | null;
  activated_at: string | null;
  validated_at: string | null;
  activation_id: string | null;
  trial_started_at: string | null;
}
