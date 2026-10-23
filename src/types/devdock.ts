export type IpVersion = 4 | 6 | null;

export interface PortListener {
  pid: number;
  processName: string;
  processStartTime: number | null;
  executablePath: string | null;
  protocol: "TCP";
  address: string;
  port: number;
  ipVersion: IpVersion;
}

export type CommandErrorCode =
  | "INVALID_ARGUMENT"
  | "SCAN_FAILED"
  | "SCAN_TIMEOUT"
  | "PROCESS_NOT_FOUND"
  | "PROCESS_IDENTITY_CHANGED"
  | "PROCESS_UNVERIFIABLE"
  | "PROCESS_PROTECTED"
  | "PERMISSION_DENIED"
  | "SIGNAL_FAILED"
  | "HTTP_URL_NOT_ALLOWED"
  | "INTERNAL_ERROR";

export interface CommandError {
  code: CommandErrorCode;
  message: string;
}

export interface FavoritePort {
  port: number;
  label: string;
  healthUrl: string | null;
}

export interface DevDockSettings {
  schemaVersion: 1;
  favorites: FavoritePort[];
}

export type HealthStatus = "healthy" | "http_error" | "network_error" | "timeout";
export interface HealthCheckResult {
  url: string;
  status: HealthStatus;
  statusCode: number | null;
  latencyMs: number | null;
  checkedAt: number;
  message: string | null;
}
