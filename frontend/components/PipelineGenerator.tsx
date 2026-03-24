import { useState } from 'react';
import { Sparkles, Loader2, Save, Copy, Check, FileCode2 } from 'lucide-react';
import { generatePipelineConfig, saveGeneratedPipeline } from '../services/api';
import type { GeneratedPipeline, PipelineFormat } from '../types';

interface PipelineGeneratorProps {
  projectPath: string;
  projectInfo: string;
}

const FORMAT_OPTIONS: { value: PipelineFormat; label: string; description: string }[] = [
  { value: 'chibby', label: 'Chibby', description: 'Native TOML pipeline' },
  { value: 'github_actions', label: 'GitHub Actions', description: '.github/workflows/ci.yml' },
  { value: 'circle_ci', label: 'CircleCI', description: '.circleci/config.yml' },
  { value: 'drone', label: 'Drone', description: '.drone.yml' },
];

export default function PipelineGenerator({ projectPath, projectInfo }: PipelineGeneratorProps) {
  const [format, setFormat] = useState<PipelineFormat>('chibby');
  const [result, setResult] = useState<GeneratedPipeline | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleGenerate() {
    setLoading(true);
    setError(null);
    setResult(null);
    setSaved(false);
    try {
      const pipeline = await generatePipelineConfig(projectPath, format, projectInfo);
      setResult(pipeline);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleSave() {
    if (!result) return;
    setSaving(true);
    try {
      await saveGeneratedPipeline(projectPath, result.file_path, result.content);
      setSaved(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  function handleCopy() {
    if (!result) return;
    navigator.clipboard.writeText(result.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="border border-[var(--color-border)] rounded-lg overflow-hidden">
      <div className="px-4 py-3 bg-[var(--color-bg-secondary)] flex items-center gap-2">
        <FileCode2 size={16} className="text-purple-400" />
        <span className="font-medium text-sm">Generate Pipeline</span>
      </div>

      <div className="p-4">
        {/* Format picker */}
        <div className="grid grid-cols-2 gap-2 mb-4">
          {FORMAT_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setFormat(opt.value)}
              className={`p-3 rounded-md border text-left text-sm transition-colors ${
                format === opt.value
                  ? 'border-purple-500 bg-purple-500/10'
                  : 'border-[var(--color-border)] hover:border-[var(--color-border-hover)]'
              }`}
            >
              <div className="font-medium">{opt.label}</div>
              <div className="text-xs text-[var(--color-text-secondary)]">{opt.description}</div>
            </button>
          ))}
        </div>

        {/* Generate button */}
        <button
          onClick={handleGenerate}
          disabled={loading}
          className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-purple-600 hover:bg-purple-700 disabled:opacity-50 text-white rounded-md text-sm transition-colors"
        >
          {loading ? <Loader2 size={14} className="animate-spin" /> : <Sparkles size={14} />}
          {loading ? 'Generating...' : 'Generate Pipeline'}
        </button>

        {/* Error */}
        {error && (
          <div className="mt-3 text-red-400 text-sm bg-red-400/10 rounded-md p-3">{error}</div>
        )}

        {/* Result */}
        {result && (
          <div className="mt-4">
            {/* File path */}
            <div className="flex items-center gap-2 mb-2">
              <code className="text-xs bg-[var(--color-bg-tertiary)] px-2 py-1 rounded">
                {result.file_path}
              </code>
              <button
                onClick={handleCopy}
                className="p-1 hover:bg-[var(--color-bg-tertiary)] rounded"
                title="Copy content"
              >
                {copied ? <Check size={12} className="text-green-400" /> : <Copy size={12} />}
              </button>
            </div>

            {/* Preview */}
            <div className="bg-[var(--color-bg-tertiary)] rounded-md p-3 max-h-64 overflow-y-auto mb-3">
              <pre className="text-xs font-mono whitespace-pre-wrap">{result.content}</pre>
            </div>

            {/* Explanation */}
            {result.explanation && (
              <div className="text-sm text-[var(--color-text-secondary)] mb-3">
                <pre className="whitespace-pre-wrap font-mono text-xs">{result.explanation}</pre>
              </div>
            )}

            {/* Save button */}
            <button
              onClick={handleSave}
              disabled={saving || saved}
              className={`w-full flex items-center justify-center gap-2 px-4 py-2 rounded-md text-sm transition-colors ${
                saved
                  ? 'bg-green-600 text-white'
                  : 'bg-[var(--color-bg-secondary)] border border-[var(--color-border)] hover:bg-[var(--color-bg-tertiary)]'
              }`}
            >
              {saved ? (
                <>
                  <Check size={14} /> Saved to {result.file_path}
                </>
              ) : saving ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <>
                  <Save size={14} /> Save to Project
                </>
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
