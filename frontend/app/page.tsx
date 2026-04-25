"use client";

import { useState, useEffect } from "react";
import DkgPanel from "@/app/components/dkg-panel";
import WalletsPanel from "@/app/components/wallets-panel";
import TransactionsPanel from "@/app/components/transactions-panel";
import { getDkgStatus, type DkgStatus } from "@/app/lib/api";

type Tab = "dkg" | "wallets" | "signing";

const TABS: { id: Tab; label: string }[] = [
  { id: "dkg", label: "DKG" },
  { id: "wallets", label: "Wallets" },
  { id: "signing", label: "Signing" },
];

export default function Home() {
  const [activeTab, setActiveTab] = useState<Tab>("dkg");
  const [dkgComplete, setDkgComplete] = useState(false);
  const [selectedWalletIndex, setSelectedWalletIndex] = useState<number | null>(
    null,
  );
  const [connectionOk, setConnectionOk] = useState<boolean | null>(null);

  // Poll DKG status at the page level so Wallets tab knows when DKG is done
  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const status: DkgStatus = await getDkgStatus();
        if (!cancelled) {
          setDkgComplete(status.status === "complete");
          setConnectionOk(true);
        }
      } catch {
        if (!cancelled) setConnectionOk(false);
      }
    };

    void poll();
    const interval = setInterval(() => void poll(), 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  return (
    <div className="flex min-h-screen flex-col">
      {/* Top Navigation Bar */}
      <header className="sticky top-0 z-50 border-b border-surface-border bg-surface/80 backdrop-blur-xl">
        <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-6">
          {/* Logo / Title */}
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-frost-600">
              <svg
                className="h-4 w-4 text-white"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth="2"
                  d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
                />
              </svg>
            </div>
            <span className="text-base font-semibold tracking-tight text-text-primary">
              FROST TSS Wallet
            </span>
            <span className="ml-1 rounded bg-surface-overlay px-1.5 py-0.5 text-[10px] font-medium text-text-tertiary">
              DEMO
            </span>
          </div>

          {/* Tab Navigation */}
          <nav className="flex h-full items-center gap-1">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`flex h-full items-center px-4 text-sm font-medium transition-colors ${
                  activeTab === tab.id ? "tab-active" : "tab-inactive"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </nav>

          {/* Right side: network indicator */}
          <div className="flex items-center gap-2">
            <div
              className={`h-2 w-2 rounded-full ${
                connectionOk === null
                  ? "bg-accent-yellow"
                  : connectionOk
                    ? "bg-accent-green"
                    : "bg-accent-red"
              }`}
            />
            <span className="text-xs text-text-secondary">
              {connectionOk === null
                ? "Connecting..."
                : connectionOk
                  ? "Devnet"
                  : "Offline"}
            </span>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="mx-auto w-full max-w-7xl flex-1 px-6 py-8">
        {activeTab === "dkg" && <DkgPanel onDkgComplete={() => setDkgComplete(true)} />}
        {activeTab === "wallets" && (
          <WalletsPanel
            dkgComplete={dkgComplete}
            selectedWalletIndex={selectedWalletIndex}
            onSelectWallet={setSelectedWalletIndex}
          />
        )}
        {activeTab === "signing" && (
          <TransactionsPanel
            dkgComplete={dkgComplete}
            selectedWalletIndex={selectedWalletIndex}
          />
        )}
      </main>

      {/* Footer */}
      <footer className="border-t border-surface-border-subtle">
        <div className="mx-auto flex h-10 max-w-7xl items-center justify-between px-6">
          <span className="text-[11px] text-text-muted">
            FROST Threshold Signature Scheme Demo
          </span>
          <span className="text-[11px] text-text-muted">Solana Devnet</span>
        </div>
      </footer>
    </div>
  );
}
