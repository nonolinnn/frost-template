"use client";

import { useState, useEffect, useCallback } from "react";
import {
  getDkgStatus,
  startDkg,
  executeDkgRound,
  formatApiError,
  type DkgStatus,
  type NodeRounds,
} from "@/app/lib/api";

const ROUND_LABELS: Record<number, { name: string; description: string }> = {
  1: { name: "Round 1", description: "Commitments" },
  2: { name: "Round 2", description: "Secret Shares" },
  3: { name: "Round 3", description: "Finalize" },
};

const NODE_CONFIGS: { id: string; label: string; participant: string; colorBg: string; colorText: string }[] = [
  { id: "node-a", label: "A", participant: "Participant 1", colorBg: "bg-frost-600/15", colorText: "text-frost-400" },
  { id: "node-b", label: "B", participant: "Participant 2", colorBg: "bg-purple-600/15", colorText: "text-purple-400" },
];

function CheckIcon() {
  return (
    <svg className="h-3.5 w-3.5 text-accent-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.5" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function SpinnerIcon({ className = "h-3.5 w-3.5" }: { className?: string }) {
  return (
    <svg className={`spinner ${className} text-frost-400`} fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
    </svg>
  );
}

function CopyIcon() {
  return (
    <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
    </svg>
  );
}

function CopyCheckIcon() {
  return (
    <svg className="h-4 w-4 text-accent-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function getNodeCompletedCount(rounds: NodeRounds): number {
  let count = 0;
  if (rounds.round1 === "complete") count++;
  if (rounds.round2 === "complete") count++;
  if (rounds.round3 === "complete") count++;
  return count;
}

function getTotalCompleted(nodes: Record<string, NodeRounds>): number {
  let total = 0;
  for (const rounds of Object.values(nodes)) {
    total += getNodeCompletedCount(rounds);
  }
  return total;
}

/**
 * Determine which round is actionable for a given node.
 * Returns the round number (1, 2, or 3) or null if none is actionable.
 */
function getActionableRound(
  nodeId: string,
  nodes: Record<string, NodeRounds>,
): number | null {
  const nodeRounds = nodes[nodeId];
  if (!nodeRounds) return null;

  const otherNodeId = nodeId === "node-a" ? "node-b" : "node-a";
  const otherRounds = nodes[otherNodeId];

  // Round 1: always actionable if pending (DKG initialized is enough)
  if (nodeRounds.round1 === "pending") return 1;

  // Round 2: this node completed R1, other node completed R1
  if (
    nodeRounds.round1 === "complete" &&
    nodeRounds.round2 === "pending" &&
    otherRounds &&
    otherRounds.round1 === "complete"
  ) {
    return 2;
  }

  // Round 3: this node completed R2, other node completed R2
  if (
    nodeRounds.round2 === "complete" &&
    nodeRounds.round3 === "pending" &&
    otherRounds &&
    otherRounds.round2 === "complete"
  ) {
    return 3;
  }

  return null;
}

interface DkgPanelProps {
  onDkgComplete?: () => void;
}

export default function DkgPanel({ onDkgComplete }: DkgPanelProps) {
  const [dkgStatus, setDkgStatus] = useState<DkgStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [executingRound, setExecutingRound] = useState<string | null>(null); // "node-a-2" format
  const [copied, setCopied] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const status = await getDkgStatus();
      setDkgStatus(status);
      if (status.status === "complete") {
        onDkgComplete?.();
      }
      setError(null);
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setLoading(false);
    }
  }, [onDkgComplete]);

  useEffect(() => {
    fetchStatus();
    // Poll every 5 seconds
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  const handleStartDkg = async () => {
    setError(null);
    try {
      const result = await startDkg();
      setDkgStatus({
        session_id: result.session_id,
        status: "initialized",
        created_at: result.created_at,
        completed_at: null,
        group_public_key: null,
        nodes: result.nodes,
      });
    } catch (err) {
      setError(formatApiError(err));
    }
  };

  const handleExecuteRound = async (nodeId: string, round: number) => {
    const key = `${nodeId}-${round}`;
    setExecutingRound(key);
    setError(null);
    try {
      const result = await executeDkgRound(round, nodeId);
      setDkgStatus((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          status: result.dkg_complete ? "complete" : "in_progress",
          group_public_key: result.group_public_key ?? prev.group_public_key,
          nodes: result.nodes,
        };
      });
      if (result.dkg_complete) {
        onDkgComplete?.();
      }
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setExecutingRound(null);
    }
  };

  const handleCopyKey = async (key: string) => {
    try {
      await navigator.clipboard.writeText(key);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard not available
    }
  };

  const isNotStarted = !dkgStatus || dkgStatus.status === "not_started";
  const isComplete = dkgStatus?.status === "complete";
  const totalCompleted = dkgStatus?.nodes ? getTotalCompleted(dkgStatus.nodes) : 0;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <SpinnerIcon className="h-6 w-6" />
        <span className="ml-3 text-sm text-text-secondary">Loading DKG status...</span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-text-primary">Distributed Key Generation</h1>
          <p className="mt-1 text-sm text-text-secondary">
            Generate a shared FROST key pair across two nodes using a 3-round protocol.
          </p>
        </div>
        {isNotStarted && (
          <button
            onClick={handleStartDkg}
            className="rounded-lg bg-frost-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-frost-700 focus:outline-none focus:ring-2 focus:ring-frost-500 focus:ring-offset-2 focus:ring-offset-surface"
          >
            Start New DKG
          </button>
        )}
      </div>

      {/* Error Banner */}
      {error && (
        <div className="rounded-lg border border-accent-red/30 bg-accent-red/5 px-4 py-3">
          <div className="flex items-center gap-2">
            <svg className="h-4 w-4 text-accent-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p className="text-sm text-accent-red">{error}</p>
          </div>
        </div>
      )}

      {/* Not Started State */}
      {isNotStarted && !error && (
        <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-12 text-center">
          <svg className="mx-auto h-10 w-10 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
          </svg>
          <p className="mt-4 text-sm text-text-secondary">
            No DKG session exists. Click &quot;Start New DKG&quot; to begin the key generation process.
          </p>
        </div>
      )}

      {/* Active DKG Session */}
      {!isNotStarted && (
        <>
          {/* Overall Progress Bar */}
          <div className="rounded-xl border border-surface-border bg-surface-raised p-5">
            <div className="mb-3 flex items-center justify-between">
              <span className="text-sm font-medium text-text-secondary">Overall Progress</span>
              <span className="text-sm font-medium text-text-primary">
                {totalCompleted} <span className="text-text-tertiary">/ 6 steps</span>
              </span>
            </div>
            <div className="flex gap-1.5">
              {Array.from({ length: 6 }, (_, i) => {
                let bgClass: string;
                if (i < totalCompleted) {
                  bgClass = "bg-accent-green";
                } else if (i === totalCompleted && !isComplete) {
                  bgClass = "bg-accent-blue badge-pulse";
                } else {
                  bgClass = "bg-surface-overlay";
                }
                return (
                  <div
                    key={i}
                    className={`h-2 flex-1 rounded-full transition-colors duration-300 ${bgClass}`}
                  />
                );
              })}
            </div>
          </div>

          {/* Node Panels */}
          <div className="grid grid-cols-2 gap-6">
            {NODE_CONFIGS.map((node) => {
              const nodeRounds = dkgStatus?.nodes?.[node.id];
              if (!nodeRounds) return null;

              const completedCount = getNodeCompletedCount(nodeRounds);
              const actionableRound = getActionableRound(node.id, dkgStatus!.nodes);
              const allComplete = completedCount === 3;

              return (
                <div key={node.id} className="rounded-xl border border-surface-border bg-surface-raised p-6">
                  {/* Node Header */}
                  <div className="mb-5 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${node.colorBg} ${node.colorText}`}>
                        <span className="text-sm font-semibold">{node.label}</span>
                      </div>
                      <div>
                        <h2 className="text-sm font-semibold text-text-primary">Node {node.label}</h2>
                        <p className="text-xs text-text-tertiary">{node.participant}</p>
                      </div>
                    </div>
                    {allComplete ? (
                      <span className="inline-flex items-center rounded-full bg-accent-green/10 px-2.5 py-0.5 text-xs font-medium text-accent-green">
                        3/3 complete
                      </span>
                    ) : actionableRound !== null ? (
                      <span className="badge-pulse inline-flex items-center rounded-full bg-accent-blue/10 px-2.5 py-0.5 text-xs font-medium text-accent-blue">
                        In Progress
                      </span>
                    ) : (
                      <span className="inline-flex items-center rounded-full bg-accent-green/10 px-2.5 py-0.5 text-xs font-medium text-accent-green">
                        {completedCount}/3 complete
                      </span>
                    )}
                  </div>

                  {/* Rounds */}
                  <div className="space-y-3">
                    {[1, 2, 3].map((round) => {
                      const roundKey = `round${round}` as keyof NodeRounds;
                      const status = nodeRounds[roundKey];
                      const isActionable = actionableRound === round;
                      const isExecuting = executingRound === `${node.id}-${round}`;

                      if (status === "complete") {
                        return (
                          <div key={round} className="flex items-center justify-between rounded-lg border border-surface-border-subtle bg-surface p-4">
                            <div className="flex items-center gap-3">
                              <div className="flex h-6 w-6 items-center justify-center rounded-full bg-accent-green/15">
                                <CheckIcon />
                              </div>
                              <div>
                                <p className="text-sm font-medium text-text-primary">{ROUND_LABELS[round].name}</p>
                                <p className="text-xs text-text-tertiary">{ROUND_LABELS[round].description}</p>
                              </div>
                            </div>
                            <span className="inline-flex items-center rounded-full bg-accent-green/10 px-2 py-0.5 text-[11px] font-medium text-accent-green">
                              Complete
                            </span>
                          </div>
                        );
                      }

                      if (isActionable) {
                        return (
                          <div key={round} className="flex items-center justify-between rounded-lg border border-accent-blue/30 bg-accent-blue/5 p-4 glow-blue">
                            <div className="flex items-center gap-3">
                              <div className="badge-pulse flex h-6 w-6 items-center justify-center rounded-full bg-accent-blue/20">
                                <div className="h-2 w-2 rounded-full bg-accent-blue" />
                              </div>
                              <div>
                                <p className="text-sm font-medium text-text-primary">{ROUND_LABELS[round].name}</p>
                                <p className="text-xs text-text-secondary">{ROUND_LABELS[round].description}</p>
                              </div>
                            </div>
                            <button
                              onClick={() => handleExecuteRound(node.id, round)}
                              disabled={isExecuting}
                              className="rounded-lg bg-frost-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-frost-700 disabled:cursor-not-allowed disabled:opacity-50"
                            >
                              {isExecuting ? (
                                <span className="flex items-center gap-1.5">
                                  <SpinnerIcon className="h-3 w-3" />
                                  Running...
                                </span>
                              ) : (
                                `Execute ${ROUND_LABELS[round].name}`
                              )}
                            </button>
                          </div>
                        );
                      }

                      // Waiting
                      return (
                        <div key={round} className="flex items-center justify-between rounded-lg border border-surface-border-subtle bg-surface p-4">
                          <div className="flex items-center gap-3">
                            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-surface-overlay">
                              <div className="h-2 w-2 rounded-full bg-text-muted" />
                            </div>
                            <div>
                              <p className="text-sm font-medium text-text-primary">{ROUND_LABELS[round].name}</p>
                              <p className="text-xs text-text-tertiary">{ROUND_LABELS[round].description}</p>
                            </div>
                          </div>
                          <span className="inline-flex items-center rounded-full bg-surface-overlay px-2 py-0.5 text-[11px] font-medium text-text-muted">
                            Waiting
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Master Public Key */}
          {isComplete && dkgStatus?.group_public_key ? (
            <div className="rounded-xl border border-accent-green/20 bg-surface-raised p-6 glow-green">
              <div className="mb-3 flex items-center gap-2">
                <svg className="h-4 w-4 text-accent-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
                <p className="text-sm font-medium text-accent-green">Master Public Key (DKG Complete)</p>
              </div>
              <div className="flex items-center justify-between rounded-lg border border-surface-border bg-surface px-4 py-3">
                <code className="font-mono text-sm text-text-primary">{dkgStatus.group_public_key}</code>
                <button
                  onClick={() => handleCopyKey(dkgStatus.group_public_key!)}
                  className="copy-btn ml-3 rounded-md p-1.5 text-text-tertiary transition-colors hover:bg-surface-overlay hover:text-text-secondary"
                  title="Copy to clipboard"
                >
                  {copied ? <CopyCheckIcon /> : <CopyIcon />}
                </button>
              </div>
            </div>
          ) : (
            <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-6">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <svg className="h-5 w-5 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                  </svg>
                  <div>
                    <p className="text-sm font-medium text-text-tertiary">Master Public Key</p>
                    <p className="text-xs text-text-muted">Appears after DKG completes successfully</p>
                  </div>
                </div>
                <span className="inline-flex items-center rounded-full bg-surface-overlay px-2 py-0.5 text-[11px] font-medium text-text-muted">
                  Pending
                </span>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
