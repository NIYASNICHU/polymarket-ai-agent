"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchBets, fetchJobs, type Bet, type Job } from "@/lib/api";
import { StatCard } from "@/components/StatCard";
import { HashCell } from "@/components/HashCell";
import { ago, usd, pct } from "@/lib/format";
import { useSSE } from "@/hooks/useSSE";

const DB_API = process.env.NEXT_PUBLIC_DB_API_URL ?? "https://api-production-3d43.up.railway.app";

export default function Dashboard() {
  const [bets, setBets]   = useState<Bet[]>([]);
  const [jobs, setJobs]   = useState<Job[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // ── Trade modal state ──────────────────────────────────────────────────────
  const [tradeModal, setTradeModal] = useState(false);
  const [tradeMarketId, setTradeMarketId] = useState("");
  const [tradeSide, setTradeSide] = useState<"YES" | "NO">("YES");
  const [tradeAmount, setTradeAmount] = useState("5");
  const [tradeLoading, setTradeLoading] = useState(false);
  const [tradeResult, setTradeResult] = useState<{ success: boolean; message: string; paper: boolean } | null>(null);

  // ── Mode toggle state ──────────────────────────────────────────────────────
  const [modeLoading, setModeLoading] = useState(false);

  const [config, setConfig] = useState<{
    eoa_address: string;
    proxy_address: string;
    deposit_address: string;
    paper_trading: boolean;
  } | null>(null);
  const [usdcBalance, setUsdcBalance] = useState<string>("0.00");

  const refresh = useCallback(async () => {
    try {
      const [b, j] = await Promise.all([fetchBets(100), fetchJobs(100)]);
      setBets(b);
      setJobs(j);
      setError(null);
    } catch (e: any) {
      console.error(e);
      setError(`${e.message || "Unknown error"} (Target: ${DB_API})`);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchConfig = useCallback(async () => {
    try {
      const configRes = await fetch(`${DB_API}/config`);
      if (!configRes.ok) throw new Error("Failed to fetch config");
      const configData = await configRes.json();
      setConfig(configData);

      const walletToQuery = configData.deposit_address || configData.proxy_address || "0xDb944cbfF21825eE0606880b4feb52A7E47c71cc";
      const res = await fetch("https://polygon-bor-rpc.publicnode.com", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          method: "eth_call",
          params: [{
            to: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174",
            data: "0x70a08231000000000000000000000000" + walletToQuery.toLowerCase().slice(2)
          }, "latest"],
          id: 1
        })
      });
      const data = await res.json();
      if (data && data.result) {
        const balanceInt = parseInt(data.result, 16);
        setUsdcBalance((balanceInt / 1e6).toFixed(2));
      }
    } catch (e) {
      console.error("Failed to fetch config or balance:", e);
    }
  }, []);

  useEffect(() => { refresh(); fetchConfig(); }, [refresh, fetchConfig]);

  useSSE(useCallback((event: any) => {
    if (event.type === "bets" || event.type === "jobs") refresh();
    if (event.type === "mode_change") fetchConfig();
  }, [refresh, fetchConfig]));

  // ── Execute manual trade ───────────────────────────────────────────────────
  const handleExecuteTrade = async () => {
    if (!tradeMarketId.trim()) return;
    setTradeLoading(true);
    setTradeResult(null);
    try {
      const res = await fetch(`${DB_API}/trade/execute`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          market_id: tradeMarketId.trim(),
          side: tradeSide,
          amount_usdc: parseFloat(tradeAmount),
          confirm: true,
        }),
      });
      const data = await res.json();
      setTradeResult(data);
      if (data.success) refresh();
    } catch (e: any) {
      setTradeResult({ success: false, message: e.message, paper: true });
    } finally {
      setTradeLoading(false);
    }
  };

  // ── Toggle paper/live mode ─────────────────────────────────────────────────
  const handleToggleMode = async () => {
    if (!config) return;
    const newPaper = !config.paper_trading;
    if (!newPaper && !window.confirm(
      "⚠️ You are switching to LIVE trading mode.\n\nThis will use REAL MONEY. Are you sure?"
    )) return;
    setModeLoading(true);
    try {
      await fetch(`${DB_API}/trading-mode`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ paper: newPaper }),
      });
      await fetchConfig();
    } catch (e) {
      console.error("Failed to toggle mode:", e);
    } finally {
      setModeLoading(false);
    }
  };

  const settled    = jobs.filter(j => j.status === "settled").length;
  const proving    = jobs.filter(j => j.status === "proving").length;
  const failed     = jobs.filter(j => j.status === "failed").length;
  const totalBets  = bets.length;
  const paperBets  = bets.filter(b => b.paper).length;
  const resolved   = bets.filter(b => b.outcome !== null);
  const wins       = resolved.filter(b =>
    (b.side === "YES" && b.outcome === true) ||
    (b.side === "NO"  && b.outcome === false)
  );
  const winRate    = resolved.length > 0 ? wins.length / resolved.length : null;
  const totalPnl   = bets.reduce((s, b) => s + (b.pnl_usdc ?? 0), 0);
  const recentBets = bets.slice(0, 10);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-zinc-500 text-sm font-mono animate-pulse">Loading…</div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {error && (
        <div className="bg-red-500/10 border border-red-500/20 text-red-400 px-4 py-3 rounded-lg text-sm font-mono">
          API Error: {error}
        </div>
      )}

      {/* ── Header ── */}
      <div className="flex items-start justify-between gap-4 flex-wrap">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Agent Dashboard</h1>
          <p className="text-zinc-500 text-sm mt-1 mb-2">
            Verifiable inference on Polymarket — powered by SP1 + HashKey
          </p>
          <div className="flex flex-wrap items-center gap-3 mt-3">
            {config?.eoa_address && (
              <div className="inline-flex items-center gap-2 px-3 py-1.5 bg-zinc-800/40 rounded-full border border-zinc-700/30">
                <span className="text-zinc-500 text-xs uppercase tracking-wider">EOA:</span>
                <code className="text-zinc-300 text-xs font-mono" title={config.eoa_address}>
                  {config.eoa_address.slice(0, 6)}...{config.eoa_address.slice(-4)}
                </code>
              </div>
            )}

            <div className="inline-flex items-center gap-2 px-3 py-1.5 bg-zinc-800/50 rounded-full border border-zinc-700/50">
              <span className="text-zinc-400 text-xs uppercase tracking-wider">Deposit Wallet:</span>
              <code className="text-emerald-400 text-xs font-mono" title={config?.deposit_address || "0xDb944cbfF21825eE0606880b4feb52A7E47c71cc"}>
                {(config?.deposit_address || "0xDb944cbfF21825eE0606880b4feb52A7E47c71cc").slice(0, 6)}...
                {(config?.deposit_address || "0xDb944cbfF21825eE0606880b4feb52A7E47c71cc").slice(-4)}
              </code>
            </div>

            <div className="inline-flex items-center gap-2 px-3 py-1.5 bg-zinc-800/50 rounded-full border border-zinc-700/50">
              <span className="text-zinc-400 text-xs uppercase tracking-wider">USDC.e Balance:</span>
              <code className="text-emerald-400 text-xs font-mono">${usdcBalance}</code>
            </div>

            {/* ── Mode badge + toggle ── */}
            {config && (
              <button
                onClick={handleToggleMode}
                disabled={modeLoading}
                title="Click to toggle paper / live trading"
                className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-full border transition-all cursor-pointer ${
                  config.paper_trading
                    ? "bg-amber-500/10 border-amber-500/30 text-amber-400 hover:bg-amber-500/20"
                    : "bg-emerald-500/10 border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/20"
                }`}
              >
                <span className={`w-1.5 h-1.5 rounded-full ${config.paper_trading ? "bg-amber-400" : "bg-emerald-400 animate-pulse"}`} />
                <span className="text-xs font-mono uppercase tracking-wider">
                  {modeLoading ? "…" : config.paper_trading ? "Paper Mode" : "⚡ Live Mode"}
                </span>
              </button>
            )}
          </div>
        </div>

        {/* ── Trade Now Button ── */}
        <button
          id="trade-now-btn"
          onClick={() => { setTradeModal(true); setTradeResult(null); }}
          className="flex items-center gap-2 px-5 py-2.5 bg-emerald-500 hover:bg-emerald-400 text-black font-bold rounded-xl transition-all shadow-lg shadow-emerald-500/20 hover:shadow-emerald-500/40 text-sm"
        >
          <span>⚡</span> Trade Now
        </button>
      </div>

      {/* ── Trade Modal ── */}
      {tradeModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="bg-[#181c2a] border border-white/10 rounded-2xl p-6 w-full max-w-md shadow-2xl space-y-5">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-bold">Manual Trade</h2>
              <button
                onClick={() => { setTradeModal(false); setTradeResult(null); }}
                className="text-zinc-500 hover:text-white text-xl leading-none"
              >×</button>
            </div>

            {/* Mode warning */}
            <div className={`px-3 py-2 rounded-lg text-xs font-mono ${
              config?.paper_trading
                ? "bg-amber-500/10 border border-amber-500/20 text-amber-400"
                : "bg-emerald-500/10 border border-emerald-500/20 text-emerald-400"
            }`}>
              {config?.paper_trading
                ? "⚠️ Paper mode — this trade will NOT use real money. Toggle to Live Mode first."
                : "⚡ Live mode — this trade will use REAL MONEY from your deposit wallet."}
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs text-zinc-400 uppercase tracking-wider mb-1 block">Market ID</label>
                <input
                  id="trade-market-id"
                  type="text"
                  placeholder="0x1234...abcd"
                  value={tradeMarketId}
                  onChange={e => setTradeMarketId(e.target.value)}
                  className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm font-mono text-white placeholder-zinc-600 focus:outline-none focus:border-emerald-500"
                />
                <p className="text-zinc-600 text-xs mt-1">Paste the market ID from Polymarket</p>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs text-zinc-400 uppercase tracking-wider mb-1 block">Side</label>
                  <div className="flex gap-2">
                    {(["YES", "NO"] as const).map(s => (
                      <button
                        key={s}
                        id={`trade-side-${s.toLowerCase()}`}
                        onClick={() => setTradeSide(s)}
                        className={`flex-1 py-2 rounded-lg text-sm font-bold transition-all ${
                          tradeSide === s
                            ? s === "YES" ? "bg-emerald-500 text-black" : "bg-red-500 text-white"
                            : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                        }`}
                      >
                        {s}
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <label className="text-xs text-zinc-400 uppercase tracking-wider mb-1 block">Amount (USDC)</label>
                  <input
                    id="trade-amount"
                    type="number"
                    min="0.01"
                    step="0.01"
                    value={tradeAmount}
                    onChange={e => setTradeAmount(e.target.value)}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm font-mono text-white focus:outline-none focus:border-emerald-500"
                  />
                </div>
              </div>
            </div>

            {tradeResult && (
              <div className={`px-3 py-2 rounded-lg text-sm ${
                tradeResult.success
                  ? "bg-emerald-500/10 border border-emerald-500/20 text-emerald-400"
                  : "bg-red-500/10 border border-red-500/20 text-red-400"
              }`}>
                {tradeResult.message}
              </div>
            )}

            <button
              id="trade-confirm-btn"
              onClick={handleExecuteTrade}
              disabled={tradeLoading || !tradeMarketId.trim()}
              className="w-full py-3 rounded-xl font-bold text-sm transition-all disabled:opacity-40 disabled:cursor-not-allowed bg-emerald-500 hover:bg-emerald-400 text-black shadow-lg shadow-emerald-500/20"
            >
              {tradeLoading ? "Submitting…" : config?.paper_trading ? "Log Paper Trade" : "⚡ Execute Live Trade"}
            </button>
          </div>
        </div>
      )}

      {/* ── Stats ── */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard label="Total Bets"  value={totalBets} />
        <StatCard label="Paper Bets"  value={paperBets} />
        <StatCard
          label="Win Rate"
          value={winRate !== null ? pct(winRate) : "—"}
          sub={`${wins.length} / ${resolved.length} resolved`}
          accent="green"
        />
        <StatCard
          label="Total P&L"
          value={usd(totalPnl)}
          accent={totalPnl >= 0 ? "green" : "red"}
        />
      </div>

      <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
        <StatCard label="Settled Proofs" value={settled} accent="green"  />
        <StatCard label="Proving"        value={proving} accent="violet" />
        <StatCard label="Failed"         value={failed}  accent="red"    />
      </div>

      {/* ── Recent Bets table ── */}
      <div>
        <h2 className="text-base font-semibold mb-4">Recent Bets</h2>
        <div className="bg-[#181c2a] border border-white/5 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/5 text-zinc-500 text-xs uppercase tracking-wider">
                <th className="text-left px-4 py-3">Market</th>
                <th className="text-left px-4 py-3">Side</th>
                <th className="text-left px-4 py-3">Size</th>
                <th className="text-left px-4 py-3">Conf.</th>
                <th className="text-left px-4 py-3">Attestation</th>
                <th className="text-left px-4 py-3">Tx</th>
                <th className="text-left px-4 py-3">P&L</th>
                <th className="text-left px-4 py-3">Placed</th>
                <th className="text-left px-4 py-3">Action</th>
              </tr>
            </thead>
            <tbody>
              {recentBets.map((b) => (
                <tr
                  key={b.id}
                  className="border-b border-white/5 hover:bg-white/[0.02] transition-colors"
                >
                  <td className="px-4 py-3 max-w-xs">
                    <p className="text-white truncate" title={b.question}>{b.question}</p>
                    <p className="text-zinc-600 text-xs font-mono mt-0.5">
                      {b.market_id.slice(0, 16)}…
                    </p>
                  </td>
                  <td className="px-4 py-3">
                    <span className={`font-mono font-bold text-xs px-2 py-0.5 rounded ${
                      b.side === "YES"
                        ? "bg-emerald-900 text-emerald-300"
                        : "bg-red-900 text-red-300"
                    }`}>
                      {b.side}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-zinc-300">{usd(b.size_usdc)}</td>
                  <td className="px-4 py-3 font-mono text-zinc-300">{pct(b.confidence)}</td>
                  <td className="px-4 py-3"><HashCell hash={b.attestation_hash} /></td>
                  <td className="px-4 py-3">
                    <HashCell
                      hash={b.tx_hash}
                      href={b.tx_hash
                        ? `https://testnet-explorer.hsk.xyz/tx/${b.tx_hash}`
                        : undefined}
                    />
                  </td>
                  <td className="px-4 py-3 font-mono">
                    {b.pnl_usdc !== null ? (
                      <span className={b.pnl_usdc >= 0 ? "text-emerald-400" : "text-red-400"}>
                        {usd(b.pnl_usdc)}
                      </span>
                    ) : (
                      <span className="text-zinc-600">pending</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-zinc-500 text-xs">{ago(b.placed_at)}</td>
                  <td className="px-4 py-3">
                    <button
                      onClick={() => {
                        setTradeMarketId(b.market_id);
                        setTradeSide(b.side as "YES" | "NO");
                        setTradeModal(true);
                        setTradeResult(null);
                      }}
                      className="text-xs px-2 py-1 rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-300 transition-colors"
                    >
                      Trade
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {recentBets.length === 0 && (
            <p className="text-center text-zinc-600 py-12">No bets yet</p>
          )}
        </div>
      </div>
    </div>
  );
}