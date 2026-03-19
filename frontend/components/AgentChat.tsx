import { useState, useRef, useEffect } from 'react';
import { Bot, X, Send, Loader2, Minimize2, Maximize2 } from 'lucide-react';
import { agentChat, getAgentStatus } from '../services/api';
import type { AgentSystemStatus } from '../types';

interface ChatMessage {
  role: 'user' | 'agent';
  text: string;
  skill?: string;
}

export default function AgentChat() {
  const [isOpen, setIsOpen] = useState(false);
  const [isMinimized, setIsMinimized] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<AgentSystemStatus | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getAgentStatus().then(setStatus).catch(() => {});
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  async function handleSend() {
    if (!input.trim() || loading) return;
    const msg = input.trim();
    setInput('');
    setMessages((prev) => [...prev, { role: 'user', text: msg }]);
    setLoading(true);
    try {
      const response = await agentChat(msg);
      setMessages((prev) => [
        ...prev,
        { role: 'agent', text: response.message, skill: response.skill_used },
      ]);
    } catch (err) {
      setMessages((prev) => [
        ...prev,
        { role: 'agent', text: `Error: ${String(err)}` },
      ]);
    } finally {
      setLoading(false);
    }
  }

  // Don't show the chat button if agent is not available
  if (status && !status.available) return null;

  if (!isOpen) {
    return (
      <button
        onClick={() => setIsOpen(true)}
        className="fixed bottom-6 right-6 w-12 h-12 bg-purple-600 hover:bg-purple-700 text-white rounded-full shadow-lg flex items-center justify-center transition-colors z-50"
        title="Open CI/CD Agent"
      >
        <Bot size={20} />
      </button>
    );
  }

  return (
    <div
      className={`fixed bottom-6 right-6 bg-[var(--color-bg-primary)] border border-[var(--color-border)] rounded-lg shadow-2xl z-50 flex flex-col ${
        isMinimized ? 'w-72 h-12' : 'w-96 h-[500px]'
      }`}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] rounded-t-lg">
        <Bot size={16} className="text-purple-400" />
        <span className="font-medium text-sm flex-1">CI/CD Agent</span>
        <button
          onClick={() => setIsMinimized(!isMinimized)}
          className="p-1 hover:bg-[var(--color-bg-tertiary)] rounded"
        >
          {isMinimized ? <Maximize2 size={12} /> : <Minimize2 size={12} />}
        </button>
        <button
          onClick={() => setIsOpen(false)}
          className="p-1 hover:bg-[var(--color-bg-tertiary)] rounded"
        >
          <X size={12} />
        </button>
      </div>

      {!isMinimized && (
        <>
          {/* Messages */}
          <div className="flex-1 overflow-y-auto p-3 space-y-3">
            {messages.length === 0 && (
              <div className="text-center text-[var(--color-text-secondary)] text-sm py-8">
                <Bot size={32} className="mx-auto mb-2 text-purple-400 opacity-50" />
                <p>Ask me about CI/CD, pipelines, failures, or deployments.</p>
              </div>
            )}
            {messages.map((msg, i) => (
              <div
                key={i}
                className={`text-sm ${
                  msg.role === 'user' ? 'ml-8 text-right' : 'mr-8'
                }`}
              >
                <div
                  className={`inline-block p-2 rounded-lg max-w-full text-left ${
                    msg.role === 'user'
                      ? 'bg-purple-600 text-white'
                      : 'bg-[var(--color-bg-secondary)]'
                  }`}
                >
                  <pre className="whitespace-pre-wrap font-mono text-xs">{msg.text}</pre>
                </div>
                {msg.skill && (
                  <div className="text-xs text-[var(--color-text-secondary)] mt-0.5">
                    {msg.skill.replace('_', ' ')}
                  </div>
                )}
              </div>
            ))}
            {loading && (
              <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
                <Loader2 size={14} className="animate-spin" />
                <span className="text-xs">Thinking...</span>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* Input */}
          <div className="p-3 border-t border-[var(--color-border)]">
            <div className="flex gap-2">
              <input
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                placeholder="Ask the agent..."
                className="flex-1 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md px-3 py-1.5 text-sm"
                disabled={loading}
              />
              <button
                onClick={handleSend}
                disabled={loading || !input.trim()}
                className="p-1.5 bg-purple-600 hover:bg-purple-700 disabled:opacity-50 text-white rounded-md transition-colors"
              >
                <Send size={14} />
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
