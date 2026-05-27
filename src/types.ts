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

export interface DoctorReport {
  version: string;
  database: DatabaseInfo;
  ocr: OcrInfo;
  hnsw: HnswInfo;
  content_index: ContentIndexInfo;
  index_location: string | null;
  scan_paths: ScanPath[];
  permissions: PermissionsInfo;
  os: OsInfo;
  recent_errors: string | null;
}

export interface DatabaseInfo {
  ok: boolean;
  path: string;
  size_bytes: number;
  files_indexed: number;
  content_indexed: number;
  last_updated: string | null;
  last_full_reindex: string | null;
}

export interface OcrInfo {
  binary_found: boolean;
  total_images: number;
  ocr_completed: number;
}

export interface HnswInfo {
  index_exists: boolean;
  vector_count: number;
}

export interface ContentIndexInfo {
  path: string;
  size_bytes: number;
}

export interface PermissionsInfo {
  ok: boolean;
  inaccessible: string[];
}

export interface OsInfo {
  os: string;
  arch: string;
}

export interface ScanPath {
  path: string;
  exists: boolean;
}

export type ThemePreference = "light" | "dark" | "system";
