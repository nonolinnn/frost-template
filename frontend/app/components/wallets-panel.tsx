"use client";

import { useState, useEffect, useCallback } from "react";
import {
  listWallets,
  createWallet,
  getWalletBalance,
  formatApiError,
  type Wallet,
  type WalletBalance,
} from "@/app/lib/api";

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
    <svg className="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
    </svg>
  );
}

function CopyCheckIcon() {
  return (
    <svg className="h-3.5 w-3.5 text-accent-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 4v16m8-8H4" />
    </svg>
  );
}

function RefreshIcon({ className = "h-4 w-4" }: { className?: string }) {
  return (
    <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
    </svg>
  );
}

function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 8)}...${address.slice(-4)}`;
}

function formatBalance(sol: number): string {
  return sol.toFixed(3);
}

interface WalletsPanelProps {
  dkgComplete: boolean;
  selectedWalletIndex: number | null;
  onSelectWallet: (index: number) => void;
}

export default function WalletsPanel({
  dkgComplete,
  selectedWalletIndex,
  onSelectWallet,
}: WalletsPanelProps) {
  const [wallets, setWallets] = useState<Wallet[]>([]);
  const [balances, setBalances] = useState<Record<number, WalletBalance>>({});
  const [loadingBalances, setLoadingBalances] = useState<Record<number, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [refreshingAll, setRefreshingAll] = useState(false);

  const fetchWallets = useCallback(async () => {
    if (!dkgComplete) {
      setLoading(false);
      return;
    }
    try {
      const result = await listWallets();
      setWallets(result.wallets);
      setError(null);
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setLoading(false);
    }
  }, [dkgComplete]);

  useEffect(() => {
    fetchWallets();
  }, [fetchWallets]);

  const fetchBalance = useCallback(async (index: number) => {
    setLoadingBalances((prev) => ({ ...prev, [index]: true }));
    try {
      const balance = await getWalletBalance(index);
      setBalances((prev) => ({ ...prev, [index]: balance }));
    } catch {
      // Silently fail balance fetch; show "---"
    } finally {
      setLoadingBalances((prev) => ({ ...prev, [index]: false }));
    }
  }, []);

  // Fetch balances for all wallets on load or when wallets change
  useEffect(() => {
    wallets.forEach((w) => {
      if (!(w.index in balances)) {
        fetchBalance(w.index);
      }
    });
    // Only re-fetch when wallet list changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wallets.length, fetchBalance]);

  const handleRefreshAll = async () => {
    setRefreshingAll(true);
    await Promise.all(wallets.map((w) => fetchBalance(w.index)));
    setRefreshingAll(false);
  };

  const handleCreateWallet = async () => {
    setCreating(true);
    setError(null);
    try {
      const newWallet = await createWallet();
      setWallets((prev) => [...prev, newWallet]);
      // Fetch balance for the new wallet
      fetchBalance(newWallet.index);
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setCreating(false);
    }
  };

  const handleCopy = async (address: string, index: number) => {
    try {
      await navigator.clipboard.writeText(address);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 1500);
    } catch {
      // clipboard not available
    }
  };

  // DKG not complete state
  if (!dkgComplete) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-xl font-semibold text-text-primary">Wallet Derivation</h1>
          <p className="mt-1 text-sm text-text-secondary">
            Derive Solana wallets from the FROST master key. Select a sender wallet for signing transactions.
          </p>
        </div>
        <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-12 text-center">
          <svg className="mx-auto h-10 w-10 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
          </svg>
          <p className="mt-4 text-sm text-text-secondary">
            DKG must be completed before wallet operations are available.
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
        <span className="ml-3 text-sm text-text-secondary">Loading wallets...</span>
      </div>
    );
  }

  const selectedWallet = wallets.find((w) => w.index === selectedWalletIndex);
  const selectedBalance = selectedWalletIndex !== null ? balances[selectedWalletIndex] : null;

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-text-primary">Wallet Derivation</h1>
          <p className="mt-1 text-sm text-text-secondary">
            Derive Solana wallets from the FROST master key. Select a sender wallet for signing transactions.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleRefreshAll}
            disabled={refreshingAll || wallets.length === 0}
            className="rounded-lg border border-surface-border px-3 py-2 text-sm font-medium text-text-secondary transition-colors hover:bg-surface-overlay hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-50"
            title="Refresh all balances"
          >
            <RefreshIcon className={`h-4 w-4 ${refreshingAll ? "spinner" : ""}`} />
          </button>
          <button
            onClick={handleCreateWallet}
            disabled={creating}
            className="flex items-center gap-2 rounded-lg bg-frost-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-frost-700 focus:outline-none focus:ring-2 focus:ring-frost-500 focus:ring-offset-2 focus:ring-offset-surface disabled:cursor-not-allowed disabled:opacity-50"
          >
            {creating ? (
              <>
                <SpinnerIcon className="h-4 w-4" />
                Creating...
              </>
            ) : (
              <>
                <PlusIcon />
                Create Wallet
              </>
            )}
          </button>
        </div>
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

      {/* Empty State */}
      {wallets.length === 0 && (
        <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-12 text-center">
          <svg className="mx-auto h-10 w-10 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
          </svg>
          <p className="mt-4 text-sm text-text-secondary">No wallets derived yet.</p>
          <p className="mt-1 text-xs text-text-muted">
            Click &quot;Create Wallet&quot; to derive your first Solana wallet from the FROST master key.
          </p>
        </div>
      )}

      {/* Wallet Table */}
      {wallets.length > 0 && (
        <div className="overflow-hidden rounded-xl border border-surface-border bg-surface-raised">
          {/* Table Header */}
          <div className="grid grid-cols-[60px_1fr_140px_140px] border-b border-surface-border px-6 py-3">
            <span className="text-xs font-medium uppercase tracking-wider text-text-muted">#</span>
            <span className="text-xs font-medium uppercase tracking-wider text-text-muted">Address</span>
            <span className="text-xs font-medium uppercase tracking-wider text-text-muted">Balance</span>
            <span className="text-right text-xs font-medium uppercase tracking-wider text-text-muted">Action</span>
          </div>

          {/* Rows */}
          {wallets.map((wallet) => {
            const isSelected = wallet.index === selectedWalletIndex;
            const balance = balances[wallet.index];
            const isLoadingBalance = loadingBalances[wallet.index];

            return (
              <div
                key={wallet.index}
                className={`grid grid-cols-[60px_1fr_140px_140px] items-center border-b border-surface-border-subtle px-6 py-4 transition-colors ${
                  isSelected ? "row-selected" : "hover:bg-surface-overlay/30"
                }`}
              >
                <span className="text-sm font-medium text-text-primary">{wallet.index}</span>
                <div className="flex items-center gap-2">
                  <code className="font-mono text-sm text-text-primary">
                    {truncateAddress(wallet.address)}
                  </code>
                  <button
                    onClick={() => handleCopy(wallet.address, wallet.index)}
                    className="copy-btn rounded-md p-1 text-text-muted transition-colors hover:bg-surface-overlay hover:text-text-secondary"
                    title="Copy address"
                  >
                    {copiedIndex === wallet.index ? <CopyCheckIcon /> : <CopyIcon />}
                  </button>
                </div>
                <span className={`font-mono text-sm ${balance ? "text-text-primary" : "text-text-secondary"}`}>
                  {isLoadingBalance ? (
                    <SpinnerIcon className="h-3.5 w-3.5" />
                  ) : balance ? (
                    `${formatBalance(balance.balance_sol)} SOL`
                  ) : (
                    "--- SOL"
                  )}
                </span>
                <div className="flex justify-end">
                  {isSelected ? (
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-frost-600/15 px-3 py-1 text-xs font-medium text-frost-400">
                      <svg className="h-3 w-3" fill="currentColor" viewBox="0 0 24 24">
                        <circle cx="12" cy="12" r="6" />
                      </svg>
                      Selected
                    </span>
                  ) : (
                    <button
                      onClick={() => onSelectWallet(wallet.index)}
                      className="rounded-full border border-surface-border px-3 py-1 text-xs font-medium text-text-secondary transition-colors hover:border-frost-600/50 hover:text-frost-400"
                    >
                      Select
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Selected Sender Summary */}
      {selectedWallet && (
        <div className="flex items-center gap-3 rounded-xl border border-frost-600/20 bg-frost-600/5 px-5 py-3.5">
          <svg className="h-4 w-4 text-frost-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 7l5 5m0 0l-5 5m5-5H6" />
          </svg>
          <span className="text-sm text-text-secondary">Selected Sender:</span>
          <span className="text-sm font-medium text-text-primary">Wallet #{selectedWallet.index}</span>
          <code className="font-mono text-sm text-frost-400">{truncateAddress(selectedWallet.address)}</code>
          {selectedBalance && (
            <span className="font-mono text-sm text-text-secondary">
              ({formatBalance(selectedBalance.balance_sol)} SOL)
            </span>
          )}
        </div>
      )}
    </div>
  );
}
