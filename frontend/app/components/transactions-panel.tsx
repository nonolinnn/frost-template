"use client";

export default function TransactionsPanel() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-xl font-semibold text-text-primary">
          Signing & Transactions
        </h1>
        <p className="mt-1 text-sm text-text-secondary">
          Create signing requests, execute FROST signing rounds, and broadcast
          transactions to Solana.
        </p>
      </div>

      <div className="rounded-xl border border-dashed border-surface-border bg-surface-raised/50 p-12 text-center">
        <svg
          className="mx-auto h-10 w-10 text-text-muted"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.5"
            d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"
          />
        </svg>
        <p className="mt-4 text-sm text-text-secondary">
          Transaction signing is coming soon.
        </p>
        <p className="mt-1 text-xs text-text-muted">
          This feature will be implemented in fr-009.
        </p>
      </div>
    </div>
  );
}
