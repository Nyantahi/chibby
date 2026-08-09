import { useEffect, useRef, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  Bot,
  X,
  Send,
  Loader2,
  Terminal,
  FileEdit,
  Check,
  XCircle,
  CheckCircle2,
  Eye,
  Wrench,
} from 'lucide-react';
import { getAgentStatus, listProjects } from '../services/api';
import {
  useAgentSession,
  useAgentUI,
  openDrawer,
  closeDrawer,
  sendAgentMessage,
  runToolSession,
  resolveApproval,
  setDrawerWidth,
  type AgentItem,
} from '../services/agentStore';
import { usePref, PREF_AGENT_MODE, type AgentInteractionMode } from '../services/prefs';
import type { AgentSystemStatus } from '../types';

/** Derive the session key and project id from the current route. */
function sessionForPath(pathname: string): { key: string; projectId?: string } {
  const m = pathname.match(/\/project\/([^/]+)/);
  if (m) return { key: `project:${m[1]}`, projectId: m[1] };
  return { key: 'general' };
}

function toolIcon(name: string) {
  if (name === 'run_command') return <Terminal size={14} className="agent-accent" />;
  if (name === 'edit_ci_file') return <FileEdit size={14} className="agent-accent" />;
  return <Bot size={14} className="agent-accent" />;
}

function ToolCard({ item }: { item: Extract<AgentItem, { kind: 'tool' }> }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="agent-tool">
      <button className="agent-tool-header" onClick={() => setOpen(!open)}>
        {toolIcon(item.name)}
        <code>{item.summary || item.name}</code>
        {item.status === 'running' && <Loader2 size={12} className="agent-spin" />}
        {item.status === 'ok' && <Check size={12} className="agent-ok" />}
        {item.status === 'error' && <XCircle size={12} className="agent-danger" />}
      </button>
      {open && item.output && <pre className="agent-tool-output">{item.output}</pre>}
    </div>
  );
}

function ApprovalCard({
  item,
  onResolve,
}: {
  item: Extract<AgentItem, { kind: 'approval' }>;
  onResolve: (approved: boolean) => void;
}) {
  return (
    <div className="agent-approval">
      <div className="agent-approval-title">
        Approve {item.tool === 'edit_ci_file' ? 'file edit' : 'command'}?
      </div>
      <div className="agent-approval-reason">{item.reason}</div>
      <pre>{item.summary}</pre>
      {item.resolved ? (
        <div className="agent-approval-resolved">
          {item.resolved === 'approved' ? (
            <CheckCircle2 size={12} className="agent-ok" />
          ) : (
            <XCircle size={12} className="agent-danger" />
          )}
          {item.resolved === 'approved' ? 'Approved' : 'Rejected'}
        </div>
      ) : (
        <div className="agent-approval-actions">
          <button className="btn btn-primary btn-sm" onClick={() => onResolve(true)}>
            Approve
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => onResolve(false)}>
            Reject
          </button>
        </div>
      )}
    </div>
  );
}

/**
 * Docked, resizable right-hand agent drawer. In a project context the agent can
 * run commands and edit CI/CD files (streamed + gated by autonomy mode);
 * elsewhere it's an advisory chat.
 */
export default function AgentDrawer() {
  const location = useLocation();
  const { key, projectId } = sessionForPath(location.pathname);
  const ui = useAgentUI();
  const session = useAgentSession(key);
  const [status, setStatus] = useState<AgentSystemStatus | null>(null);
  const [projectPath, setProjectPath] = useState<string | undefined>();
  const [input, setInput] = useState('');
  // Advisory-first: default to read-only Advise; user opts into Act to mutate.
  const [agentMode, setAgentMode] = usePref<AgentInteractionMode>(PREF_AGENT_MODE, 'advise');
  const advise = agentMode === 'advise';
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Re-check availability on mount and each time the drawer opens, so adding an
  // API key in Settings takes effect without an app reload.
  useEffect(() => {
    getAgentStatus()
      .then(setStatus)
      .catch(() => {});
  }, [ui.open]);

  // Resolve the project's repo path (tool sessions act on the filesystem).
  useEffect(() => {
    let cancelled = false;
    listProjects()
      .then((projects) => {
        if (cancelled) return;
        setProjectPath(
          projectId ? projects.find((p) => p.project.id === projectId)?.project.path : undefined
        );
      })
      .catch(() => {
        if (!cancelled) setProjectPath(undefined);
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [session.items, ui.open]);

  const available = !status || status.available;

  async function handleSend() {
    const text = input.trim();
    if (!text || session.running) return;
    setInput('');
    if (projectPath) {
      await runToolSession(key, text, projectPath, advise);
    } else {
      await sendAgentMessage(key, text, projectId);
    }
  }

  function handleResizeStart(e: React.MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = ui.width;
    const onMove = (ev: MouseEvent) => setDrawerWidth(startWidth + (startX - ev.clientX));
    const onUp = () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  if (!ui.open) {
    return (
      <button className="agent-fab" onClick={openDrawer} title="Open CI/CD Agent">
        <Bot size={24} />
      </button>
    );
  }

  return (
    <div className="agent-drawer" style={{ width: ui.width }}>
      <div className="agent-drawer-resize" onMouseDown={handleResizeStart} title="Drag to resize" />

      <div className="agent-drawer-body">
        {/* Header */}
        <div className="agent-drawer-header">
          <Bot size={16} className="agent-accent" />
          <span className="agent-drawer-title">CI/CD Agent</span>
          <span className="agent-drawer-tag">
            {projectPath ? (advise ? 'Advise · read-only' : 'Act') : 'Chat'}
          </span>
          <button className="agent-icon-btn" onClick={closeDrawer} title="Close">
            <X size={14} />
          </button>
        </div>

        {!available ? (
          <div className="agent-unavailable">
            <Bot size={32} />
            <p>Add an Anthropic or OpenAI API key to enable the agent.</p>
            <Link to="/settings" onClick={closeDrawer} className="btn btn-primary btn-sm">
              Open Settings
            </Link>
          </div>
        ) : (
          <>
            {/* Items */}
            <div className="agent-messages">
              {session.items.length === 0 && (
                <div className="agent-empty">
                  <Bot size={32} />
                  <p>
                    {projectPath
                      ? advise
                        ? 'Ask about this project — I read files and pipeline config to diagnose and recommend, without changing anything. Switch to Act to apply fixes.'
                        : 'Ask me to check, fix, or set up this project. I can run commands and edit CI/CD files (with approval).'
                      : 'Ask me about CI/CD, pipelines, failures, or deployments.'}
                  </p>
                </div>
              )}

              {session.items.map((item, i) => {
                if (item.kind === 'user') {
                  return (
                    <div key={i} className="agent-msg agent-msg-user">
                      <div className="agent-bubble">
                        <pre>{item.text}</pre>
                      </div>
                    </div>
                  );
                }
                if (item.kind === 'assistant') {
                  return (
                    <div key={i} className="agent-msg agent-msg-assistant">
                      <div className="agent-bubble">
                        <pre>{item.text}</pre>
                      </div>
                    </div>
                  );
                }
                if (item.kind === 'tool') return <ToolCard key={i} item={item} />;
                if (item.kind === 'approval')
                  return (
                    <ApprovalCard
                      key={i}
                      item={item}
                      onResolve={(approved) => resolveApproval(key, item.id, approved)}
                    />
                  );
                return (
                  <div key={i} className="agent-error">
                    {item.text}
                  </div>
                );
              })}

              {session.running && (
                <div className="agent-loading">
                  <Loader2 size={14} className="agent-spin" />
                  <span>Working...</span>
                </div>
              )}
              <div ref={messagesEndRef} />
            </div>

            {/* Mode toggle (project context only) */}
            {projectPath && (
              <div className="agent-mode-row">
                <div className="tabs" role="tablist" aria-label="Agent mode">
                  <button
                    type="button"
                    className={`tab ${advise ? 'tab-active' : ''}`}
                    onClick={() => setAgentMode('advise')}
                    disabled={session.running}
                    aria-pressed={advise}
                    title="Read-only: investigate and recommend without changing anything"
                  >
                    <Eye size={14} /> Advise
                  </button>
                  <button
                    type="button"
                    className={`tab ${!advise ? 'tab-active' : ''}`}
                    onClick={() => setAgentMode('act')}
                    disabled={session.running}
                    aria-pressed={!advise}
                    title="Run commands and edit CI/CD files (with approval)"
                  >
                    <Wrench size={14} /> Act
                  </button>
                </div>
              </div>
            )}

            {/* Input */}
            <div className="agent-input-row">
              <input
                className="input"
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                placeholder={
                  projectPath
                    ? advise
                      ? 'Ask a question or describe the issue...'
                      : 'Ask the agent to do something...'
                    : 'Ask the agent...'
                }
                disabled={session.running}
              />
              <button
                className="btn btn-primary"
                onClick={handleSend}
                disabled={session.running || !input.trim()}
              >
                <Send size={14} />
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
