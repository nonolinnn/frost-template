"use client";

import { useState, useEffect, useCallback } from "react";
import {
  listSigningRequests,
  createSigningRequest,
  getSigningRequest,
  executeSigningRound,
  aggregateAndBroadcast,
  listWallets,
  getWalletBalance,
  formatApiError,
  type SigningRequest,
  type SigningRequestStatus,
  type SigningNodeRounds,
  type Wallet,
  type WalletBalance,
} from "@/app/lib/api";

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

function SpinnerIcon({ className = "h-3.5 w-3.5" }: { className?: string }) {
  return (
    <svg className={`spinner ${className} text-frost-400`} fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
    </svg>
  );
}

function CheckIcon({ className = "h-3.5 w-3.5 text-accent-green" }: { className?: string }) {
  return (
    <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.5" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function ArrowRightIcon({ className = "h-3 w-3 text-text-muted" }: { className?: string }) {
  return (
    <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M17 8l4 4m0 0l-4 4m4-4H3" />
    </svg>
  );
}

function ExternalLinkIcon() {
  return (
    <svg className="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
    </svg>
  );
}

function ChevronDownIcon() {
  return (
    <svg className="h-4 w-4 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7" />
    </svg>
  );
}

function SuccessCircleIcon() {
  return (
    <svg className="h-4 w-4 text-accent-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function ErrorIcon() {
  return (
    <svg className="h-4 w-4 text-accent-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 8)}...${address.slice(-4)}`;
}

function truncateHash(hash: string): string {
  if (hash.length <= 16) return hash;
  return `${hash.slice(0, 12)}...${hash.slice(-4)}`;
}

function lamportsToSol(lamports: number): number {
  return lamports / 1_000_000_000;
}

function solToLamports(sol: number): number {
  return Math.round(sol * 1_000_000_000);
}

function formatSol(lamports: number): string {
  return lamportsToSol(lamports).toFixed(3);
}

function formatTime(isoString: string): string {
  const d = new Date(isoString);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** Basic Base58 character set check */
function isValidBase58(address: string): boolean {
  if (!address || address.length < 32 || address.length > 44) return false;
  return /^[1-9A-HJ-NP-Za-km-z]+$/.test(address);
}

const NODE_CONFIGS = [
  { id: "node-a", label: "A", colorBg: "bg-frost-600/15", colorText: "text-frost-400" },
  { id: "node-b", label: "B", colorBg: "bg-purple-600/15", colorText: "text-purple-400" },
] as const;

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

type StatusBadgeVariant = "waiting" | "in_progress" | "complete" | "error";

const STATUS_DISPLAY: Record<SigningRequestStatus, { label: string; variant: StatusBadgeVariant }> = {
  pending: { label: "Pending", variant: "waiting" },
  round1_in_progress: { label: "Round 1", variant: "in_progress" },
  round2_in_progress: { label: "Round 2", variant: "in_progress" },
  aggregating: { label: "Aggregating", variant: "in_progress" },
  broadcasted: { label: "Broadcasted", variant: "complete" },
  confirmed: { label: "Confirmed", variant: "complete" },
  failed: { label: "Failed", variant: "error" },
};

function StatusBadge({ status, compact = false }: { status: SigningRequestStatus; compact?: boolean }) {
  const { label, variant } = STATUS_DISPLAY[status];
  const sizeClass = compact ? "text-[10px] px-2 py-0.5" : "text-xs px-2.5 py-0.5";

  const variantClasses: Record<StatusBadgeVariant, string> = {
    waiting: "bg-surface-overlay text-text-muted",
    in_progress: "bg-accent-blue/10 text-accent-blue badge-pulse",
    complete: "bg-accent-green/10 text-accent-green",
    error: "bg-accent-red/10 text-accent-red",
  };

  return (
    <span className={`inline-flex items-center rounded-full font-medium ${sizeClass} ${variantClasses[variant]}`}>
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Timeline Stepper
// ---------------------------------------------------------------------------

const TIMELINE_STEPS: { key: string; label: string; statuses: SigningRequestStatus[] }[] = [
  { key: "pending", label: "Pending", statuses: ["pending"] },
  { key: "round1", label: "Round 1", statuses: ["round1_in_progress"] },
  { key: "round2", label: "Round 2", statuses: ["round2_in_progress"] },
  { key: "aggregate", label: "Aggregate", statuses: ["aggregating"] },
  { key: "broadcast", label: "Broadcast", statuses: ["broadcasted"] },
  { key: "confirmed", label: "Confirmed", statuses: ["confirmed"] },
];

function getStepIndex(status: SigningRequestStatus): number {
  if (status === "failed") return -1;
  for (let i = 0; i < TIMELINE_STEPS.length; i++) {
    if (TIMELINE_STEPS[i].statuses.includes(status)) return i;
  }
  return 0;
}

function StatusTimeline({ status }: { status: SigningRequestStatus }) {
  const currentIdx = getStepIndex(status);
  const isFailed = status === "failed";

  return (
    <div className="mb-6">
      <h3 className="mb-3 text-xs font-medium uppercase tracking-wider text-text-muted">Status Timeline</h3>
      <div className="flex items-center gap-0">
        {TIMELINE_STEPS.map((step, i) => {
          const isComplete = !isFailed && currentIdx > i;
          const isActive = !isFailed && currentIdx === i;
          const isWaiting = isFailed || currentIdx < i;

          return (
            <div key={step.key} className="contents">
              {/* Step circle + label */}
              <div className="flex flex-col items-center">
                {isComplete ? (
                  <div className="flex h-7 w-7 items-center justify-center rounded-full bg-accent-green/15">
                    <CheckIcon />
                  </div>
                ) : isActive ? (
                  <div className="badge-pulse flex h-7 w-7 items-center justify-center rounded-full bg-accent-blue/20">
                    <div className="h-2.5 w-2.5 rounded-full bg-accent-blue" />
                  </div>
                ) : (
                  <div className="flex h-7 w-7 items-center justify-center rounded-full bg-surface-overlay">
                    <div className="h-2 w-2 rounded-full bg-text-muted" />
                  </div>
                )}
                <span
                  className={`mt-1.5 text-[10px] ${
                    isComplete
                      ? "text-accent-green"
                      : isActive
                        ? "font-medium text-accent-blue"
                        : "text-text-muted"
                  }`}
                >
                  {step.label}
                </span>
              </div>

              {/* Connector line */}
              {i < TIMELINE_STEPS.length - 1 && (
                <div
                  className={`mx-1 h-0.5 flex-1 ${
                    !isFailed && currentIdx > i + 1
                      ? "bg-accent-green"
                      : !isFailed && currentIdx === i + 1
                        ? "bg-gradient-to-r from-accent-green to-accent-blue"
                        : !isFailed && currentIdx === i
                          ? "bg-gradient-to-r from-accent-blue to-surface-border"
                          : "bg-surface-border"
                  }`}
                />
              )}
            </div>
          );
        })}
      </div>

      {/* Failed state message */}
      {isFailed && (
        <div className="mt-3 flex items-center gap-2 rounded-lg border border-accent-red/20 bg-accent-red/5 px-3 py-2">
          <ErrorIcon />
          <span className="text-xs text-accent-red">Transaction failed</span>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Signing Node Panels
// ---------------------------------------------------------------------------

function getSigningActionableRound(
  nodeId: string,
  requestStatus: SigningRequestStatus,
  nodes: Record<string, SigningNodeRounds>,
): number | null {
  const nodeRounds = nodes[nodeId];
  if (!nodeRounds) return null;

  const otherNodeId = nodeId === "node-a" ? "node-b" : "node-a";
  const otherRounds = nodes[otherNodeId];

  // Cannot trigger rounds in terminal states
  if (requestStatus === "aggregating" || requestStatus === "broadcasted" || requestStatus === "confirmed" || requestStatus === "failed") {
    return null;
  }

  // Round 1: actionable if pending and request is pending or round1_in_progress
  if (
    nodeRounds.round1 === "pending" &&
    (requestStatus === "pending" || requestStatus === "round1_in_progress")
  ) {
    return 1;
  }

  // Round 2: actionable if round1 complete, round2 pending, both nodes completed round1,
  // and request is round1_in_progress or round2_in_progress
  if (
    nodeRounds.round1 === "complete" &&
    nodeRounds.round2 === "pending" &&
    otherRounds &&
    otherRounds.round1 === "complete" &&
    (requestStatus === "round1_in_progress" || requestStatus === "round2_in_progress")
  ) {
    return 2;
  }

  return null;
}

function canAggregate(
  requestStatus: SigningRequestStatus,
  nodes: Record<string, SigningNodeRounds>,
): boolean {
  if (requestStatus !== "round2_in_progress") return false;
  return NODE_CONFIGS.every((n) => {
    const nr = nodes[n.id];
    return nr && nr.round1 === "complete" && nr.round2 === "complete";
  });
}

interface SigningNodePanelsProps {
  request: SigningRequest;
  executingKey: string | null;
  onExecuteRound: (nodeId: string, round: number) => void;
}

function SigningNodePanels({ request, executingKey, onExecuteRound }: SigningNodePanelsProps) {
  return (
    <div className="mb-6 grid grid-cols-2 gap-4">
      {NODE_CONFIGS.map((node) => {
        const nodeRounds = request.nodes[node.id];
        if (!nodeRounds) return null;
        const actionableRound = getSigningActionableRound(node.id, request.status, request.nodes);

        return (
          <div key={node.id} className="rounded-lg border border-surface-border bg-surface p-4">
            {/* Node header */}
            <div className="mb-3 flex items-center gap-2">
              <div className={`flex h-7 w-7 items-center justify-center rounded-md ${node.colorBg} ${node.colorText}`}>
                <span className="text-xs font-semibold">{node.label}</span>
              </div>
              <span className="text-sm font-medium text-text-primary">Node {node.label}</span>
            </div>

            {/* Rounds */}
            <div className="space-y-2">
              {[1, 2].map((round) => {
                const roundKey = `round${round}` as keyof SigningNodeRounds;
                const roundStatus = nodeRounds[roundKey];
                const isActionable = actionableRound === round;
                const isExecuting = executingKey === `${node.id}-${round}`;

                if (roundStatus === "complete") {
                  return (
                    <div key={round} className="flex items-center justify-between rounded-md bg-surface-raised px-3 py-2">
                      <div className="flex items-center gap-2">
                        <CheckIcon />
                        <span className="text-xs text-text-primary">Round {round}</span>
                      </div>
                      <span className="text-[10px] text-accent-green">Done</span>
                    </div>
                  );
                }

                if (isActionable) {
                  return (
                    <div key={round} className="flex items-center justify-between rounded-md border border-accent-blue/30 bg-accent-blue/5 px-3 py-2">
                      <div className="flex items-center gap-2">
                        <div className="badge-pulse h-3 w-3 rounded-full bg-accent-blue/30">
                          <div className="mx-auto mt-[3px] h-1.5 w-1.5 rounded-full bg-accent-blue" />
                        </div>
                        <span className="text-xs text-text-primary">Round {round}</span>
                      </div>
                      <button
                        onClick={() => onExecuteRound(node.id, round)}
                        disabled={isExecuting}
                        className="rounded-md bg-frost-600 px-2.5 py-1 text-[10px] font-medium text-white hover:bg-frost-700 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {isExecuting ? (
                          <span className="flex items-center gap-1">
                            <SpinnerIcon className="h-2.5 w-2.5" />
                            Running
                          </span>
                        ) : (
                          "Execute"
                        )}
                      </button>
                    </div>
                  );
                }

                // Waiting
                return (
                  <div key={round} className="flex items-center justify-between rounded-md bg-surface-raised px-3 py-2">
                    <div className="flex items-center gap-2">
                      <div className="h-3 w-3 rounded-full bg-surface-overlay">
                        <div className="mx-auto mt-[4px] h-1.5 w-1.5 rounded-full bg-text-muted" />
                      </div>
                      <span className="text-xs text-text-secondary">Round {round}</span>
                    </div>
                    <span className="text-[10px] text-text-muted">Waiting</span>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

interface TransactionsPanelProps {
  dkgComplete: boolean;
  selectedWalletIndex: number | null;
}

export default function TransactionsPanel({ dkgComplete, selectedWalletIndex }: TransactionsPanelProps) {
  // ---------- State ----------
  const [requests, setRequests] = useState<SigningRequest[]>([]);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [selectedRequest, setSelectedRequest] = useState<SigningRequest | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Form state
  const [wallets, setWallets] = useState<Wallet[]>([]);
  const [walletBalances, setWalletBalances] = useState<Record<number, WalletBalance>>({});
  const [senderIndex, setSenderIndex] = useState<number | null>(selectedWalletIndex);
  const [recipient, setRecipient] = useState("");
  const [amountSol, setAmountSol] = useState("");
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [showWalletDropdown, setShowWalletDropdown] = useState(false);

  // Execution state
  const [executingKey, setExecutingKey] = useState<string | null>(null);
  const [aggregating, setAggregating] = useState(false);

  // ---------- Data Fetching ----------

  const fetchRequests = useCallback(async () => {
    try {
      const result = await listSigningRequests();
      setRequests(result.signing_requests);
      setError(null);
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchSelectedRequest = useCallback(async (id: string) => {
    try {
      const req = await getSigningRequest(id);
      setSelectedRequest(req);
      // Also update in list
      setRequests((prev) =>
        prev.map((r) => (r.id === id ? req : r)),
      );
    } catch (err) {
      setError(formatApiError(err));
    }
  }, []);

  const fetchWallets = useCallback(async () => {
    try {
      const result = await listWallets();
      setWallets(result.wallets);
      // Fetch balances
      for (const w of result.wallets) {
        try {
          const bal = await getWalletBalance(w.index);
          setWalletBalances((prev) => ({ ...prev, [w.index]: bal }));
        } catch {
          // silent fail on balance
        }
      }
    } catch {
      // silent fail
    }
  }, []);

  useEffect(() => {
    if (dkgComplete) {
      fetchRequests();
      fetchWallets();
    } else {
      setLoading(false);
    }
  }, [dkgComplete, fetchRequests, fetchWallets]);

  // Poll for updates on the selected request (if in a transient state)
  useEffect(() => {
    if (!selectedRequestId) return;
    const interval = setInterval(() => {
      fetchSelectedRequest(selectedRequestId);
    }, 3000);
    return () => clearInterval(interval);
  }, [selectedRequestId, fetchSelectedRequest]);

  // Poll list
  useEffect(() => {
    if (!dkgComplete) return;
    const interval = setInterval(fetchRequests, 5000);
    return () => clearInterval(interval);
  }, [dkgComplete, fetchRequests]);

  // Sync senderIndex when selectedWalletIndex changes
  useEffect(() => {
    if (selectedWalletIndex !== null) {
      setSenderIndex(selectedWalletIndex);
    }
  }, [selectedWalletIndex]);

  // ---------- Handlers ----------

  const handleCreateRequest = async () => {
    setFormError(null);

    // Validation
    if (senderIndex === null) {
      setFormError("Please select a sender wallet.");
      return;
    }
    if (!recipient.trim()) {
      setFormError("Please enter a recipient address.");
      return;
    }
    if (!isValidBase58(recipient.trim())) {
      setFormError("Invalid recipient address. Must be a valid Base58 Solana address.");
      return;
    }
    const amount = parseFloat(amountSol);
    if (isNaN(amount) || amount <= 0) {
      setFormError("Please enter a positive amount.");
      return;
    }

    // Balance warning
    const senderBalance = walletBalances[senderIndex];
    if (senderBalance && amount > senderBalance.balance_sol) {
      setFormError(`Insufficient balance. Wallet has ${senderBalance.balance_sol.toFixed(3)} SOL.`);
      return;
    }

    setCreating(true);
    try {
      const req = await createSigningRequest(senderIndex, recipient.trim(), solToLamports(amount));
      setRequests((prev) => [req, ...prev]);
      setSelectedRequestId(req.id);
      setSelectedRequest(req);
      // Clear form
      setRecipient("");
      setAmountSol("");
    } catch (err) {
      setFormError(formatApiError(err));
    } finally {
      setCreating(false);
    }
  };

  const handleExecuteRound = async (nodeId: string, round: number) => {
    if (!selectedRequest) return;
    const key = `${nodeId}-${round}`;
    setExecutingKey(key);
    setError(null);
    try {
      const result = await executeSigningRound(selectedRequest.id, round, nodeId);
      setSelectedRequest((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          status: result.signing_request_status,
          nodes: result.nodes,
        };
      });
      // Update in list
      setRequests((prev) =>
        prev.map((r) =>
          r.id === selectedRequest.id
            ? { ...r, status: result.signing_request_status, nodes: result.nodes }
            : r,
        ),
      );
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setExecutingKey(null);
    }
  };

  const handleAggregate = async () => {
    if (!selectedRequest) return;
    setAggregating(true);
    setError(null);
    try {
      const result = await aggregateAndBroadcast(selectedRequest.id);
      setSelectedRequest((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          status: result.status,
          tx_signature: result.tx_signature,
          explorer_url: result.explorer_url,
        };
      });
      setRequests((prev) =>
        prev.map((r) =>
          r.id === selectedRequest.id
            ? { ...r, status: result.status, tx_signature: result.tx_signature, explorer_url: result.explorer_url }
            : r,
        ),
      );
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setAggregating(false);
    }
  };

  const handleSelectRequest = (req: SigningRequest) => {
    setSelectedRequestId(req.id);
    setSelectedRequest(req);
    setError(null);
  };

  // ---------- Derived ----------

  const senderWallet = wallets.find((w) => w.index === senderIndex);
  const senderBalance = senderIndex !== null ? walletBalances[senderIndex] : null;

  // ---------- DKG not complete ----------

  if (!dkgComplete) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-xl font-semibold text-text-primary">Signing & Transactions</h1>
          <p className="mt-1 text-sm text-text-secondary">
            Create signing requests, execute FROST signing rounds, and broadcast transactions to Solana.
          </p>
        </div>
        <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-12 text-center">
          <svg className="mx-auto h-10 w-10 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
          </svg>
          <p className="mt-4 text-sm text-text-secondary">
            DKG must be completed before signing operations are available.
          </p>
          <p className="mt-1 text-xs text-text-muted">
            Complete the Distributed Key Generation process on the DKG tab first.
          </p>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <SpinnerIcon className="h-6 w-6" />
        <span className="ml-3 text-sm text-text-secondary">Loading signing requests...</span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div>
        <h1 className="text-xl font-semibold text-text-primary">Signing & Transactions</h1>
        <p className="mt-1 text-sm text-text-secondary">
          Create signing requests, execute FROST signing rounds, and broadcast transactions to Solana.
        </p>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="rounded-lg border border-accent-red/30 bg-accent-red/5 px-4 py-3">
          <div className="flex items-center gap-2">
            <ErrorIcon />
            <p className="text-sm text-accent-red">{error}</p>
          </div>
        </div>
      )}

      {/* Create Signing Request Form */}
      <div className="rounded-xl border border-surface-border bg-surface-raised p-6">
        <h2 className="mb-4 text-sm font-semibold text-text-primary">Create Signing Request</h2>
        <div className="grid grid-cols-[1fr_1fr_auto] gap-4">
          {/* Sender Wallet */}
          <div>
            <label className="mb-1.5 block text-xs font-medium text-text-secondary">Sender Wallet</label>
            <div className="relative">
              <button
                type="button"
                onClick={() => setShowWalletDropdown(!showWalletDropdown)}
                className="flex w-full items-center rounded-lg border border-surface-border bg-surface px-3 py-2.5 text-left transition-colors focus:border-frost-600/50 focus:ring-1 focus:ring-frost-600/30 focus:outline-none"
              >
                {senderWallet ? (
                  <>
                    <span className="text-sm text-text-primary">Wallet #{senderWallet.index}</span>
                    <span className="mx-2 text-text-muted">-</span>
                    <code className="font-mono text-xs text-text-secondary">{truncateAddress(senderWallet.address)}</code>
                  </>
                ) : (
                  <span className="text-sm text-text-muted">Select a wallet...</span>
                )}
                <span className="ml-auto"><ChevronDownIcon /></span>
              </button>
              {showWalletDropdown && (
                <div className="absolute z-10 mt-1 w-full rounded-lg border border-surface-border bg-surface-raised shadow-lg">
                  {wallets.length === 0 ? (
                    <div className="px-3 py-2 text-xs text-text-muted">No wallets available. Derive one first.</div>
                  ) : (
                    wallets.map((w) => (
                      <button
                        key={w.index}
                        type="button"
                        onClick={() => {
                          setSenderIndex(w.index);
                          setShowWalletDropdown(false);
                        }}
                        className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-surface-overlay/50 ${
                          senderIndex === w.index ? "bg-frost-600/10 text-frost-400" : "text-text-primary"
                        }`}
                      >
                        <span className="font-medium">#{w.index}</span>
                        <code className="font-mono text-xs text-text-secondary">{truncateAddress(w.address)}</code>
                        {walletBalances[w.index] && (
                          <span className="ml-auto font-mono text-xs text-text-tertiary">
                            {walletBalances[w.index].balance_sol.toFixed(3)} SOL
                          </span>
                        )}
                      </button>
                    ))
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Recipient */}
          <div>
            <label className="mb-1.5 block text-xs font-medium text-text-secondary">Recipient Address</label>
            <input
              type="text"
              placeholder="Enter Solana address..."
              value={recipient}
              onChange={(e) => setRecipient(e.target.value)}
              className="w-full rounded-lg border border-surface-border bg-surface px-3 py-2.5 font-mono text-sm text-text-primary placeholder-text-muted outline-none transition-colors focus:border-frost-600/50 focus:ring-1 focus:ring-frost-600/30"
            />
          </div>

          {/* Amount + Submit */}
          <div className="flex items-end gap-3">
            <div>
              <label className="mb-1.5 block text-xs font-medium text-text-secondary">Amount</label>
              <div className="flex items-center rounded-lg border border-surface-border bg-surface">
                <input
                  type="text"
                  placeholder="0.00"
                  value={amountSol}
                  onChange={(e) => {
                    const val = e.target.value;
                    if (val === "" || /^\d*\.?\d*$/.test(val)) {
                      setAmountSol(val);
                    }
                  }}
                  className="w-24 bg-transparent px-3 py-2.5 text-sm text-text-primary outline-none"
                />
                <span className="pr-3 text-xs font-medium text-text-muted">SOL</span>
              </div>
            </div>
            <button
              onClick={handleCreateRequest}
              disabled={creating}
              className="whitespace-nowrap rounded-lg bg-frost-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-frost-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {creating ? (
                <span className="flex items-center gap-1.5">
                  <SpinnerIcon className="h-3.5 w-3.5" />
                  Creating...
                </span>
              ) : (
                "Create Request"
              )}
            </button>
          </div>
        </div>

        {/* Balance warning */}
        {senderBalance && (
          <p className="mt-2 text-xs text-text-tertiary">
            Available: {senderBalance.balance_sol.toFixed(3)} SOL
          </p>
        )}

        {/* Form Error */}
        {formError && (
          <div className="mt-3 flex items-center gap-2">
            <ErrorIcon />
            <p className="text-xs text-accent-red">{formError}</p>
          </div>
        )}
      </div>

      {/* Split View: Request List + Detail */}
      <div className="grid grid-cols-[340px_1fr] gap-6">

        {/* Left: Request List */}
        <div className="rounded-xl border border-surface-border bg-surface-raised">
          <div className="border-b border-surface-border px-4 py-3">
            <span className="text-xs font-medium uppercase tracking-wider text-text-muted">Signing Requests</span>
          </div>
          {requests.length === 0 ? (
            <div className="px-4 py-8 text-center">
              <p className="text-xs text-text-muted">No signing requests yet.</p>
              <p className="mt-1 text-xs text-text-muted">Create one above to get started.</p>
            </div>
          ) : (
            <div className="divide-y divide-surface-border-subtle">
              {requests.map((req, i) => {
                const isActive = selectedRequestId === req.id;
                return (
                  <button
                    key={req.id}
                    type="button"
                    onClick={() => handleSelectRequest(req)}
                    className={`w-full cursor-pointer px-4 py-3.5 text-left transition-colors ${
                      isActive
                        ? "border-l-[3px] border-l-accent-blue bg-accent-blue/[0.06]"
                        : "border-l-[3px] border-l-transparent hover:bg-white/[0.03]"
                    }`}
                  >
                    <div className="mb-1.5 flex items-center justify-between">
                      <span className="text-sm font-medium text-text-primary">
                        #{requests.length - i} &mdash; {formatSol(req.amount_lamports)} SOL
                      </span>
                      <StatusBadge status={req.status} compact />
                    </div>
                    <div className="flex items-center gap-1.5 text-xs text-text-tertiary">
                      <code className="font-mono">{truncateAddress(req.sender_address).slice(0, 8)}</code>
                      <ArrowRightIcon />
                      <code className="font-mono">{truncateAddress(req.recipient).slice(0, 8)}</code>
                      <span className="ml-auto text-text-muted">{formatTime(req.created_at)}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* Right: Request Detail */}
        <div className="rounded-xl border border-surface-border bg-surface-raised p-6">
          {!selectedRequest ? (
            <div className="flex h-full items-center justify-center py-16">
              <p className="text-sm text-text-muted">Select a signing request to view details</p>
            </div>
          ) : (
            <>
              {/* Detail Header */}
              <div className="mb-6 flex items-center justify-between">
                <div>
                  <h2 className="text-base font-semibold text-text-primary">
                    Request #{requests.findIndex((r) => r.id === selectedRequest.id) >= 0
                      ? requests.length - requests.findIndex((r) => r.id === selectedRequest.id)
                      : "?"}
                  </h2>
                  <p className="mt-0.5 text-sm text-text-secondary">
                    {formatSol(selectedRequest.amount_lamports)} SOL transfer
                  </p>
                </div>
                <StatusBadge status={selectedRequest.status} />
              </div>

              {/* Transfer Summary */}
              <div className="mb-6 flex items-center gap-4 rounded-lg border border-surface-border-subtle bg-surface px-4 py-3">
                <div className="text-center">
                  <p className="text-[10px] uppercase tracking-wider text-text-muted">From</p>
                  <code className="font-mono text-xs text-text-primary">{truncateAddress(selectedRequest.sender_address)}</code>
                </div>
                <div className="flex items-center gap-2">
                  <div className="h-px w-8 bg-surface-border" />
                  <span className="rounded-md bg-frost-600/10 px-2 py-0.5 font-mono text-xs font-medium text-frost-400">
                    {formatSol(selectedRequest.amount_lamports)} SOL
                  </span>
                  <div className="h-px w-8 bg-surface-border" />
                  <ArrowRightIcon className="h-4 w-4 text-text-muted" />
                </div>
                <div className="text-center">
                  <p className="text-[10px] uppercase tracking-wider text-text-muted">To</p>
                  <code className="font-mono text-xs text-text-primary">{truncateAddress(selectedRequest.recipient)}</code>
                </div>
              </div>

              {/* Status Timeline */}
              <StatusTimeline status={selectedRequest.status} />

              {/* Node Signing Panels */}
              <SigningNodePanels
                request={selectedRequest}
                executingKey={executingKey}
                onExecuteRound={handleExecuteRound}
              />

              {/* Aggregate & Broadcast */}
              <div className="flex items-center gap-4">
                <button
                  onClick={handleAggregate}
                  disabled={!canAggregate(selectedRequest.status, selectedRequest.nodes) || aggregating}
                  className={`flex-1 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors ${
                    canAggregate(selectedRequest.status, selectedRequest.nodes) && !aggregating
                      ? "bg-frost-600 text-white hover:bg-frost-700"
                      : "cursor-not-allowed border border-surface-border bg-surface text-text-muted"
                  }`}
                >
                  {aggregating ? (
                    <span className="flex items-center justify-center gap-2">
                      <SpinnerIcon className="h-4 w-4" />
                      Aggregating...
                    </span>
                  ) : (
                    "Aggregate & Broadcast"
                  )}
                </button>
              </div>

              {/* Transaction Result */}
              {selectedRequest.tx_signature && (
                <div className="mt-4 rounded-lg border border-accent-green/20 bg-accent-green/5 p-4">
                  <div className="mb-2 flex items-center gap-2">
                    <SuccessCircleIcon />
                    <span className="text-sm font-medium text-accent-green">
                      {selectedRequest.status === "confirmed" ? "Transaction Confirmed" : "Transaction Broadcasted"}
                    </span>
                  </div>
                  <div className="flex items-center justify-between rounded-md border border-surface-border-subtle bg-surface px-3 py-2">
                    <code className="font-mono text-xs text-text-primary">
                      {truncateHash(selectedRequest.tx_signature)}
                    </code>
                    <a
                      href={`https://explorer.solana.com/tx/${selectedRequest.tx_signature}?cluster=devnet`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1.5 text-xs font-medium text-frost-400 hover:text-frost-300"
                    >
                      View on Explorer
                      <ExternalLinkIcon />
                    </a>
                  </div>
                </div>
              )}

              {/* Error message from backend */}
              {selectedRequest.status === "failed" && selectedRequest.error_message && (
                <div className="mt-4 rounded-lg border border-accent-red/20 bg-accent-red/5 p-4">
                  <div className="flex items-center gap-2">
                    <ErrorIcon />
                    <span className="text-sm text-accent-red">{selectedRequest.error_message}</span>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
