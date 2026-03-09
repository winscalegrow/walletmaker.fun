"use client";

import React, { useState, useEffect, useRef } from 'react';
import DigitalSerenity from '@/components/ui/digital-serenity-animated-landing-page';
import { SmokeBackground } from '@/components/ui/spooky-smoke-animation';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from "@/components/ui/card";
import { Loader2, Copy, CheckCircle2 } from "lucide-react";

export default function Home() {
  const [prefix, setPrefix] = useState('');
  const [isGrinding, setIsGrinding] = useState(false);
  const [result, setResult] = useState<{ public_key: string; private_key: string } | null>(null);
  const [attempts, setAttempts] = useState(0);
  const [startTime, setStartTime] = useState<number>(0);
  const [timeElapsed, setTimeElapsed] = useState(0);
  const [errorMsg, setErrorMsg] = useState('');
  const [copied, setCopied] = useState(false);

  const [isReady, setIsReady] = useState(false);

  const workerRef = useRef<Worker | null>(null);
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const targetPrefixRef = useRef<string>('');

  useEffect(() => {
    // Initialize Web Worker with type: 'module'
    const workerUrl = new URL('./grinderWorker.ts', import.meta.url);
    workerRef.current = new Worker(workerUrl, { type: 'module' });

    workerRef.current.onmessage = (e) => {
      const { type, data, error } = e.data;

      if (type === 'READY') {
        setIsReady(true);
      } else if (type === 'MATCH') {
        setIsGrinding(false);
        setResult(data);
        if (timerRef.current) clearInterval(timerRef.current);
      } else if (type === 'BATCH_DONE') {
        // Keep grinding
        setAttempts(prev => prev + 20000);
        workerRef.current?.postMessage({ type: 'GRIND', prefix: targetPrefixRef.current, batchSize: 20000 });
      } else if (type === 'VALIDATE_OK') {
        setErrorMsg('');
        workerRef.current?.postMessage({ type: 'GRIND', prefix: targetPrefixRef.current, batchSize: 20000 });
      } else if (type === 'VALIDATE_ERR') {
        setErrorMsg(error);
        setIsGrinding(false);
      } else if (type === 'GRIND_ERR' || type === 'INIT_ERR') {
        setErrorMsg(error || "An unknown error occurred.");
        setIsGrinding(false);
      }
    };

    workerRef.current.postMessage({ type: 'INIT', origin: window.location.origin });

    return () => {
      workerRef.current?.terminate();
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const handleStart = () => {
    const trimmed = prefix.trim();
    if (!trimmed) return;
    targetPrefixRef.current = trimmed;
    setIsGrinding(true);
    setResult(null);
    setAttempts(0);
    setStartTime(Date.now());
    setTimeElapsed(0);
    setErrorMsg('');
    setCopied(false);

    // Timer for UI
    timerRef.current = setInterval(() => {
      setTimeElapsed(Date.now());
    }, 100);

    // Validate and let the worker start grinding automatically if VALIDATE_OK
    workerRef.current?.postMessage({ type: 'VALIDATE', prefix: trimmed });
  };

  const handleStop = () => {
    setIsGrinding(false);
    if (timerRef.current) clearInterval(timerRef.current);
  };

  const handleCopy = () => {
    if (result) {
      navigator.clipboard.writeText(`Address: ${result.public_key}\nPrivate Key: ${result.private_key}`);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <main className="relative min-h-screen bg-black text-white font-sans overflow-hidden">
      <SmokeBackground smokeColor="#1E293B" />

      <DigitalSerenity>
        <Card className="w-full max-w-lg bg-black/40 backdrop-blur-xl border border-slate-800/50 shadow-2xl p-6 rounded-2xl z-50">
          <CardContent className="space-y-6 pt-6 flex flex-col items-center">

            <div className="w-full space-y-2">
              <label className="text-sm font-medium text-slate-300">Desired Prefix (Case-Insensitive)</label>
              <Input
                value={prefix}
                onChange={(e) => setPrefix(e.target.value)}
                placeholder="e.g. MOON (max 6 chars)"
                maxLength={6}
                disabled={isGrinding}
                className="bg-slate-900/50 border-slate-700 text-slate-100 placeholder:text-slate-500 text-center text-lg tracking-widest uppercase font-mono h-14"
              />
              {errorMsg && <p className="text-red-400 text-xs mt-1 text-center font-medium animate-pulse">{errorMsg}</p>}
            </div>

            <div className="w-full flex justify-center mt-4">
              {!isGrinding ? (
                <Button
                  onClick={handleStart}
                  disabled={!prefix || prefix.length > 6 || !isReady}
                  className="w-full bg-slate-100 hover:bg-white text-slate-900 font-bold tracking-wider rounded-xl h-12 transition-all hover:scale-[1.02] disabled:opacity-50"
                >
                  {!isReady ? "LOADING ENGINE..." : "START GENERATING"}
                </Button>
              ) : (
                <Button
                  onClick={handleStop}
                  variant="destructive"
                  className="w-full font-bold tracking-wider rounded-xl h-12 transition-all hover:scale-[1.02]"
                >
                  <Loader2 className="mr-2 h-5 w-5 animate-spin" />
                  STOP GENERATING
                </Button>
              )}
            </div>

            {(isGrinding || attempts > 0) && !result && (
              <div className="w-full text-center space-y-2 py-4 border-t border-slate-800/50 mt-6">
                <div className="text-slate-400 text-sm font-mono">
                  {attempts.toLocaleString()} keypairs checked
                </div>
                <div className="text-slate-500 text-xs font-mono">
                  Elapsed: {startTime > 0 ? ((timeElapsed - startTime) / 1000).toFixed(1) : "0.0"}s •
                  Rate: {startTime > 0 && timeElapsed - startTime > 0 ? Math.round(attempts / ((timeElapsed - startTime) / 1000)).toLocaleString() : 0} keys/s
                </div>
              </div>
            )}

            {result && (
              <div className="w-full space-y-4 py-4 border-t border-slate-800/50 mt-6 animate-in slide-in-from-bottom-4 fade-in duration-500">
                <div className="flex justify-between items-center mb-2">
                  <h3 className="text-emerald-400 font-bold tracking-widest text-sm flex items-center">
                    <CheckCircle2 className="w-4 h-4 mr-1" /> MATCH FOUND
                  </h3>
                  <Button variant="ghost" size="sm" onClick={handleCopy} className="text-slate-400 hover:text-white hover:bg-slate-800">
                    {copied ? <CheckCircle2 className="w-4 h-4 mr-1 text-emerald-400" /> : <Copy className="w-4 h-4 mr-1" />}
                    {copied ? 'COPIED' : 'COPY ALL'}
                  </Button>
                </div>

                <div className="space-y-1">
                  <label className="text-xs text-slate-500 uppercase tracking-widest">Public Address</label>
                  <div className="break-all font-mono text-xs bg-slate-900/60 border border-slate-700/50 p-3 rounded-lg text-slate-300">
                    <span className="text-white font-bold">{result.public_key.slice(0, prefix.length)}</span>
                    {result.public_key.slice(prefix.length)}
                  </div>
                </div>

                <div className="space-y-1">
                  <label className="text-xs text-slate-500 uppercase tracking-widest mt-3 flex">Private Key (Keep Secret)</label>
                  <div className="break-all font-mono text-xs bg-slate-950 border border-red-500/20 p-3 rounded-lg text-red-300 opacity-80 hover:opacity-100 transition-opacity">
                    {result.private_key}
                  </div>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </DigitalSerenity>
    </main>
  );
}
