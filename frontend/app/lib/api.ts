// Coordinator API client
// All endpoints from the API contract (Frontend to Coordinator)

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

// ---------- Types ----------

export type RoundStatus = "pending" | "complete";

export interface NodeRounds {
  round1: RoundStatus;
  round2: RoundStatus;
  round3: RoundStatus;
}

export interface DkgStatus {
  session_id: string | null;
  status: "not_started" | "initialized" | "in_progress" | "complete";
  created_at: string | null;
  completed_at: string | null;
  group_public_key: string | null;
  nodes: Record<string, NodeRounds>;
}

export interface DkgRoundResponse {
  session_id: string;
  node_id: string;
  round: number;
  status: string;
  dkg_complete?: boolean;
  group_public_key?: string;
  nodes: Record<string, NodeRounds>;
}

export interface DkgStartResponse {
  session_id: string;
  status: string;
  created_at: string;
  nodes: Record<string, NodeRounds>;
}

export interface Wallet {
  index: number;
  address: string;
  public_key: string;
  created_at: string;
}

export interface WalletListResponse {
  wallets: Wallet[];
}

export interface WalletBalance {
  index: number;
  address: string;
  balance_lamports: number;
  balance_sol: number;
}

export interface ApiError {
  error: {
    code: string;
    message: string;
  };
}

// ---------- Helpers ----------

class ApiRequestError extends Error {
  code: string;
  status: number;

  constructor(message: string, code: string, status: number) {
    super(message);
    this.name = "ApiRequestError";
    this.code = code;
    this.status = status;
  }
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      headers: { "Content-Type": "application/json" },
      ...options,
    });
  } catch (err) {
    throw new ApiRequestError(
      "Unable to connect to the coordinator. Is the backend running?",
      "CONNECTION_ERROR",
      0,
    );
  }

  if (!response.ok) {
    let body: ApiError | undefined;
    try {
      body = (await response.json()) as ApiError;
    } catch {
      // response body is not JSON
    }
    throw new ApiRequestError(
      body?.error?.message ?? `Request failed with status ${response.status}`,
      body?.error?.code ?? "UNKNOWN_ERROR",
      response.status,
    );
  }

  return (await response.json()) as T;
}

// ---------- DKG ----------

export async function startDkg(): Promise<DkgStartResponse> {
  return request<DkgStartResponse>("/api/dkg/start", { method: "POST" });
}

export async function getDkgStatus(): Promise<DkgStatus> {
  return request<DkgStatus>("/api/dkg/status");
}

export async function executeDkgRound(
  round: number,
  nodeId: string,
): Promise<DkgRoundResponse> {
  return request<DkgRoundResponse>(
    `/api/dkg/round/${round}/node/${nodeId}`,
    { method: "POST" },
  );
}

// ---------- Wallets ----------

export async function createWallet(): Promise<Wallet> {
  return request<Wallet>("/api/wallets", { method: "POST" });
}

export async function listWallets(): Promise<WalletListResponse> {
  return request<WalletListResponse>("/api/wallets");
}

export async function getWalletBalance(index: number): Promise<WalletBalance> {
  return request<WalletBalance>(`/api/wallets/${index}/balance`);
}

// ---------- Signing Requests ----------

export type SigningRequestStatus =
  | "pending"
  | "round1_in_progress"
  | "round2_in_progress"
  | "aggregating"
  | "broadcasted"
  | "confirmed"
  | "failed";

export interface SigningNodeRounds {
  round1: RoundStatus;
  round2: RoundStatus;
}

export interface SigningRequest {
  id: string;
  wallet_index: number;
  sender_address: string;
  recipient: string;
  amount_lamports: number;
  status: SigningRequestStatus;
  created_at: string;
  updated_at?: string;
  tx_signature: string | null;
  explorer_url: string | null;
  error_message?: string | null;
  nodes: Record<string, SigningNodeRounds>;
}

export interface SigningRequestListResponse {
  signing_requests: SigningRequest[];
}

export interface SigningRoundResponse {
  signing_request_id: string;
  node_id: string;
  round: number;
  status: string;
  signing_request_status: SigningRequestStatus;
  nodes: Record<string, SigningNodeRounds>;
}

export interface AggregateResponse {
  signing_request_id: string;
  status: SigningRequestStatus;
  tx_signature: string;
  explorer_url: string;
}

export async function createSigningRequest(
  walletIndex: number,
  recipient: string,
  amountLamports: number,
): Promise<SigningRequest> {
  return request<SigningRequest>("/api/signing-requests", {
    method: "POST",
    body: JSON.stringify({
      wallet_index: walletIndex,
      recipient,
      amount_lamports: amountLamports,
    }),
  });
}

export async function listSigningRequests(): Promise<SigningRequestListResponse> {
  return request<SigningRequestListResponse>("/api/signing-requests");
}

export async function getSigningRequest(id: string): Promise<SigningRequest> {
  return request<SigningRequest>(`/api/signing-requests/${id}`);
}

export async function executeSigningRound(
  requestId: string,
  round: number,
  nodeId: string,
): Promise<SigningRoundResponse> {
  return request<SigningRoundResponse>(
    `/api/signing-requests/${requestId}/round/${round}/node/${nodeId}`,
    { method: "POST" },
  );
}

export async function aggregateAndBroadcast(
  requestId: string,
): Promise<AggregateResponse> {
  return request<AggregateResponse>(
    `/api/signing-requests/${requestId}/aggregate`,
    { method: "POST" },
  );
}

// ---------- Error helpers ----------

export function isApiError(err: unknown): err is ApiRequestError {
  return err instanceof ApiRequestError;
}

export function formatApiError(err: unknown): string {
  if (isApiError(err)) {
    return err.message;
  }
  if (err instanceof Error) {
    return err.message;
  }
  return "An unexpected error occurred";
}
