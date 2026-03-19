import { useState } from 'react';
import {
  Bot,
  ChevronDown,
  ChevronUp,
  Copy,
  AlertTriangle,
  AlertCircle,
  Info,
  Loader2,
  Sparkles,
} from 'lucide-react';
import { analyzeRun, agentChat, getAgentStatus } from '../services/api';
import type { AgentAnalysis, AgentResponse, Finding, Severity } from '../types';

interface AgentPanelProps {
  runId: string;
  projectId?: string;
  isFailed: boolean;
}

function severityIcon(severity: Severity) {
  switch (severity) {
    case 'critical':
      return <AlertCircle size={16} className="text-red-400" />;
    case 'warning':
      return <AlertTriangle size={16} className="text-yellow-400" />;
    case 'info':
      return <Info size={16} className="text-blue-400" />;
  }
}

function severityBorder(severity: Severity): string {
  switch (severity) {
    case 'critical':
      return 'border-red-500/30';
    case 'warning':
      return 'border-yellow-500/30';
    case 'info':
      return 'border-blue-500/30';
  }
}

function FindingCard({ finding }: { finding: Finding }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      className={`border rounded-md p-3 mb-2 bg-[var(--color-bg-secondary)] ${severityBorder(finding.severity)}`}
    >
      <div
        className="flex items-center gap-2 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        {severityIcon(finding.severity)}
        <span className="flex-1 font-medium text-sm">{finding.title}</span>
        {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </div>
      {expanded && (
        <div className="mt-2 text-sm text-[var(--color-text-secondary)]">
          <pre className="whitespace-pre-wrap font-mono text-xs">{finding.detail}</pre>
          {finding.suggested_command && (
            <div className="mt-2 flex items-center gap-2">
              <code className="bg-[var(--color-bg-tertiary)] px-2 py-1 rounded text-xs">
                {finding.suggested_command}
              </code>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  navigator.clipboard.writeText(finding.suggested_command!);
                }}
                className="p-1 hover:bg-[var(--color-bg-tertiary)] rounded"
                title="Copy command"
              >
                <Copy size={12} />
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function AgentPanel({ runId, projectId, isFailed }: AgentPanelProps) {
  const [analysis, setAnalysis] = useState<AgentAnalysis | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(false);

  // Chat state
  const [chatInput, setChatInput] = useState('');
  const [chatMessages, setChatMessages] = useState<
    { role: 'user' | 'agent'; text: string }[]
  >([]);
  const [chatLoading, setChatLoading] = useState(false);

  async function handleAnalyze() {
    setLoading(true);
    setError(null);
    try {
      const result = await analyzeRun(runId);
      setAnalysis(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleChat() {
    if (!chatInput.trim()) return;
    const msg = chatInput.trim();
    setChatInput('');
    setChatMessages((prev) => [...prev, { role: 'user', text: msg }]);
    setChatLoading(true);
    try {
      const response = await agentChat(msg, projectId, runId);
      setChatMessages((prev) => [...prev, { role: 'agent', text: response.message }]);
    } catch (err) {
      setChatMessages((prev) => [
        ...prev,
        { role: 'agent', text: `Error: ${String(err)}` },
      ]);
    } finally {
      setChatLoading(false);
    }
  }

  return (
    <div className="mt-4 border border-[var(--color-border)] rounded-lg overflow-hidden">
      {/* Header */}
      <div
        className="flex items-center gap-2 px-4 py-2 bg-[var(--color-bg-secondary)] cursor-pointer"
        onClick={() => setCollapsed(!collapsed)}
      >
        <Bot size={16} className="text-purple-400" />
        <span className="font-medium text-sm flex-1">CI/CD Agent</span>
        {analysis && (
          <span className="text-xs text-[var(--color-text-secondary)]">
            {analysis.findings.length} finding{analysis.findings.length !== 1 ? 's' : ''}
          </span>
        )}
        {collapsed ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
      </div>

      {!collapsed && (
        <div className="p-4">
          {/* Analyze button */}
          {!analysis && !loading && (
            <div className="text-center">
              <button
                onClick={handleAnalyze}
                className="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-md text-sm transition-colors"
              >
                <Sparkles size={14} />
                {isFailed ? 'Analyze Failure' : 'Analyze Run'}
              </button>
              <p className="text-xs text-[var(--color-text-secondary)] mt-2">
                Ask the agent to analyze this pipeline run
              </p>
            </div>
          )}

          {/* Loading */}
          {loading && (
            <div className="flex items-center gap-2 justify-center py-4">
              <Loader2 size={16} className="animate-spin text-purple-400" />
              <span className="text-sm text-[var(--color-text-secondary)]">Analyzing...</span>
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="text-red-400 text-sm bg-red-400/10 rounded-md p-3">{error}</div>
          )}

          {/* Analysis results */}
          {analysis && (
            <div>
              {/* Summary */}
              <p className="text-sm mb-3">{analysis.summary}</p>

              {/* Findings */}
              {analysis.findings.map((f, i) => (
                <FindingCard key={i} finding={f} />
              ))}

              {/* Suggested actions */}
              {analysis.suggested_actions.length > 0 && (
                <div className="mt-3">
                  <h4 className="text-xs font-medium text-[var(--color-text-secondary)] uppercase mb-1">
                    Suggested Actions
                  </h4>
                  <ol className="list-decimal list-inside text-sm space-y-1">
                    {analysis.suggested_actions.map((action, i) => (
                      <li key={i}>{action}</li>
                    ))}
                  </ol>
                </div>
              )}

              {/* Follow-up chat */}
              <div className="mt-4 border-t border-[var(--color-border)] pt-3">
                <h4 className="text-xs font-medium text-[var(--color-text-secondary)] uppercase mb-2">
                  Ask Follow-up
                </h4>

                {/* Chat messages */}
                {chatMessages.length > 0 && (
                  <div className="space-y-2 mb-3 max-h-48 overflow-y-auto">
                    {chatMessages.map((msg, i) => (
                      <div
                        key={i}
                        className={`text-sm p-2 rounded ${
                          msg.role === 'user'
                            ? 'bg-[var(--color-bg-tertiary)] ml-8'
                            : 'bg-purple-500/10 mr-8'
                        }`}
                      >
                        <pre className="whitespace-pre-wrap font-mono text-xs">{msg.text}</pre>
                      </div>
                    ))}
                  </div>
                )}

                <div className="flex gap-2">
                  <input
                    type="text"
                    value={chatInput}
                    onChange={(e) => setChatInput(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleChat()}
                    placeholder="Ask about this run..."
                    className="flex-1 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md px-3 py-1.5 text-sm"
                    disabled={chatLoading}
                  />
                  <button
                    onClick={handleChat}
                    disabled={chatLoading || !chatInput.trim()}
                    className="px-3 py-1.5 bg-purple-600 hover:bg-purple-700 disabled:opacity-50 text-white rounded-md text-sm transition-colors"
                  >
                    {chatLoading ? <Loader2 size={14} className="animate-spin" /> : 'Ask'}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
