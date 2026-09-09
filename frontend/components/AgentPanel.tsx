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
import { analyzeRun, agentChat } from '../services/api';
import type { AgentAnalysis, ChatTurn, Finding, Severity } from '../types';

interface AgentPanelProps {
  runId: string;
  projectId?: string;
  isFailed: boolean;
}

function severityIcon(severity: Severity) {
  switch (severity) {
    case 'critical':
      return <AlertCircle size={16} className="agent-danger" />;
    case 'warning':
      return <AlertTriangle size={16} className="agent-accent" />;
    case 'info':
      return <Info size={16} className="agent-accent" />;
  }
}

function FindingCard({ finding }: { finding: Finding }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={`agent-finding sev-${finding.severity}`}>
      <div className="agent-finding-head" onClick={() => setExpanded(!expanded)}>
        {severityIcon(finding.severity)}
        <span className="agent-finding-title">{finding.title}</span>
        {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </div>
      {expanded && (
        <div className="agent-finding-detail">
          <pre>{finding.detail}</pre>
          {finding.suggested_command && (
            <div className="agent-finding-cmd">
              <code>{finding.suggested_command}</code>
              <button
                className="agent-icon-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  navigator.clipboard.writeText(finding.suggested_command!);
                }}
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

  const [chatInput, setChatInput] = useState('');
  const [chatMessages, setChatMessages] = useState<{ role: 'user' | 'agent'; text: string }[]>([]);
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
    const history: ChatTurn[] = chatMessages.map((m) => ({
      role: m.role === 'agent' ? 'assistant' : 'user',
      content: m.text,
    }));
    setChatMessages((prev) => [...prev, { role: 'user', text: msg }]);
    setChatLoading(true);
    try {
      const response = await agentChat(msg, history, projectId, runId);
      setChatMessages((prev) => [...prev, { role: 'agent', text: response.message }]);
    } catch (err) {
      setChatMessages((prev) => [...prev, { role: 'agent', text: `Error: ${String(err)}` }]);
    } finally {
      setChatLoading(false);
    }
  }

  return (
    <div className="agent-panel">
      <div className="agent-panel-header" onClick={() => setCollapsed(!collapsed)}>
        <Bot size={16} className="agent-accent" />
        <span className="agent-panel-title">CI/CD Agent</span>
        {analysis && (
          <span className="agent-drawer-tag">
            {analysis.findings.length} finding{analysis.findings.length !== 1 ? 's' : ''}
          </span>
        )}
        {collapsed ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
      </div>

      {!collapsed && (
        <div className="agent-panel-body">
          {!analysis && !loading && (
            <div className="agent-panel-center">
              <button className="btn btn-primary btn-sm" onClick={handleAnalyze}>
                <Sparkles size={14} />
                {isFailed ? 'Analyze Failure' : 'Analyze Run'}
              </button>
              <p className="agent-drawer-tag" style={{ marginTop: 'var(--space-sm)' }}>
                Ask the agent to analyze this pipeline run
              </p>
            </div>
          )}

          {loading && (
            <div className="agent-loading" style={{ justifyContent: 'center' }}>
              <Loader2 size={16} className="agent-spin" />
              <span>Analyzing...</span>
            </div>
          )}

          {error && <div className="agent-error">{error}</div>}

          {analysis && (
            <div>
              <p className="agent-msg" style={{ marginBottom: 'var(--space-md)' }}>
                {analysis.summary}
              </p>

              {analysis.findings.map((f, i) => (
                <FindingCard key={i} finding={f} />
              ))}

              {analysis.suggested_actions.length > 0 && (
                <div style={{ marginTop: 'var(--space-md)' }}>
                  <h4 className="agent-drawer-tag">Suggested Actions</h4>
                  <ol style={{ paddingLeft: 'var(--space-lg)' }}>
                    {analysis.suggested_actions.map((action, i) => (
                      <li key={i} className="agent-msg">
                        {action}
                      </li>
                    ))}
                  </ol>
                </div>
              )}

              {/* Follow-up chat */}
              <div
                style={{
                  marginTop: 'var(--space-lg)',
                  borderTop: '1px solid var(--color-border)',
                  paddingTop: 'var(--space-md)',
                }}
              >
                <h4 className="agent-drawer-tag">Ask Follow-up</h4>

                {chatMessages.length > 0 && (
                  <div
                    className="agent-messages"
                    style={{ maxHeight: 200, padding: 0, marginBottom: 'var(--space-sm)' }}
                  >
                    {chatMessages.map((msg, i) => (
                      <div
                        key={i}
                        className={`agent-msg ${msg.role === 'user' ? 'agent-msg-user' : 'agent-msg-assistant'}`}
                      >
                        <div className="agent-bubble">
                          <pre>{msg.text}</pre>
                        </div>
                      </div>
                    ))}
                  </div>
                )}

                <div className="agent-input-row" style={{ padding: 0, border: 'none' }}>
                  <input
                    className="input"
                    type="text"
                    value={chatInput}
                    onChange={(e) => setChatInput(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleChat()}
                    placeholder="Ask about this run..."
                    disabled={chatLoading}
                  />
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={handleChat}
                    disabled={chatLoading || !chatInput.trim()}
                  >
                    {chatLoading ? <Loader2 size={14} className="agent-spin" /> : 'Ask'}
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
