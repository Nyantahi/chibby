import {
  AlertTriangle,
  CheckCircle,
  Info,
  AlertCircle,
  ExternalLink,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
import { useState } from 'react';
import type {
  ProjectRecommendations,
  FileRecommendation,
  RecommendationPriority,
  RecommendationCategory,
} from '../types';

interface RecommendationsPanelProps {
  recommendations: ProjectRecommendations | null;
  loading?: boolean;
}

/** Map priority to CSS class and icon */
function getPriorityConfig(priority: RecommendationPriority) {
  switch (priority) {
    case 'critical':
      return { className: 'recommendation-critical', icon: AlertCircle, label: 'Critical' };
    case 'high':
      return { className: 'recommendation-high', icon: AlertTriangle, label: 'High' };
    case 'medium':
      return { className: 'recommendation-medium', icon: Info, label: 'Medium' };
    case 'low':
      return { className: 'recommendation-low', icon: CheckCircle, label: 'Low' };
  }
}

/** Format category for display */
function formatCategory(category: RecommendationCategory): string {
  const labels: Record<RecommendationCategory, string> = {
    version_control: 'Version Control',
    documentation: 'Documentation',
    ci_cd: 'CI/CD',
    code_quality: 'Code Quality',
    testing: 'Testing',
    security: 'Security',
    container: 'Container',
    dependencies: 'Dependencies',
  };
  return labels[category] || category;
}

/** Group recommendations by priority */
function groupByPriority(
  recommendations: FileRecommendation[]
): Record<RecommendationPriority, FileRecommendation[]> {
  const groups: Record<RecommendationPriority, FileRecommendation[]> = {
    critical: [],
    high: [],
    medium: [],
    low: [],
  };

  recommendations.forEach((rec) => {
    groups[rec.priority].push(rec);
  });

  return groups;
}

function RecommendationItem({ recommendation }: { recommendation: FileRecommendation }) {
  const config = getPriorityConfig(recommendation.priority);
  const Icon = config.icon;

  return (
    <li className={`recommendation-item ${config.className}`}>
      <div className="recommendation-header">
        <Icon size={14} className="recommendation-icon" />
        <span className="recommendation-filename">{recommendation.file_name}</span>
        <span className={`badge badge-${recommendation.priority}`}>{config.label}</span>
      </div>
      <p className="recommendation-reason">{recommendation.reason}</p>
      <div className="recommendation-meta">
        <span className="recommendation-category">{formatCategory(recommendation.category)}</span>
        {recommendation.docs_url && (
          <a
            href={recommendation.docs_url}
            target="_blank"
            rel="noopener noreferrer"
            className="recommendation-docs-link"
          >
            Learn more <ExternalLink size={12} />
          </a>
        )}
      </div>
    </li>
  );
}

function PrioritySection({
  priority,
  recommendations,
  defaultExpanded = false,
}: {
  priority: RecommendationPriority;
  recommendations: FileRecommendation[];
  defaultExpanded?: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const config = getPriorityConfig(priority);
  const Icon = config.icon;

  if (recommendations.length === 0) return null;

  return (
    <div className={`recommendation-section ${config.className}`}>
      <button className="recommendation-section-header" onClick={() => setExpanded(!expanded)}>
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <Icon size={14} />
        <span>{config.label}</span>
        <span className={`badge badge-${priority}`}>{recommendations.length}</span>
      </button>
      {expanded && (
        <ul className="recommendation-list">
          {recommendations.map((rec, i) => (
            <RecommendationItem key={`${rec.file_name}-${i}`} recommendation={rec} />
          ))}
        </ul>
      )}
    </div>
  );
}

function RecommendationsPanel({ recommendations, loading }: RecommendationsPanelProps) {
  if (loading) {
    return (
      <div className="project-sidebar-card recommendations-panel">
        <h4 className="project-sidebar-title">
          <AlertTriangle size={14} />
          Recommendations
        </h4>
        <p className="text-muted text-xs">Analyzing project...</p>
      </div>
    );
  }

  if (!recommendations) {
    return null;
  }

  const { summary, readiness_score } = recommendations;
  const grouped = groupByPriority(recommendations.recommendations);
  const totalMissing =
    summary.critical_missing + summary.high_missing + summary.medium_missing + summary.low_missing;

  // Calculate score color
  const getScoreClass = (score: number): string => {
    if (score >= 80) return 'score-excellent';
    if (score >= 60) return 'score-good';
    if (score >= 40) return 'score-fair';
    return 'score-poor';
  };

  return (
    <div className="project-sidebar-card recommendations-panel">
      <h4 className="project-sidebar-title">
        <AlertTriangle size={14} />
        Recommendations
        <span className="badge badge-neutral">{totalMissing}</span>
      </h4>

      {totalMissing === 0 ? (
        <div className="recommendations-perfect">
          <CheckCircle size={24} className="text-success" />
          <p className="text-success">Great job! Your project has all recommended files.</p>
        </div>
      ) : (
        <>
          {/* Readiness Score */}
          <div className="readiness-score-container">
            <div className="readiness-score-label">CI/CD Readiness</div>
            <div className={`readiness-score-value ${getScoreClass(readiness_score)}`}>
              {readiness_score}%
            </div>
            <div className="readiness-score-bar">
              <div
                className={`readiness-score-fill ${getScoreClass(readiness_score)}`}
                style={{ width: `${readiness_score}%` }}
              />
            </div>
          </div>

          {/* Summary badges */}
          <div className="recommendation-summary">
            {summary.critical_missing > 0 && (
              <span className="badge badge-critical">{summary.critical_missing} Critical</span>
            )}
            {summary.high_missing > 0 && (
              <span className="badge badge-high">{summary.high_missing} High</span>
            )}
            {summary.medium_missing > 0 && (
              <span className="badge badge-medium">{summary.medium_missing} Medium</span>
            )}
            {summary.low_missing > 0 && (
              <span className="badge badge-low">{summary.low_missing} Low</span>
            )}
          </div>

          {/* Project types detected */}
          {recommendations.project_types.length > 0 && (
            <div className="detected-project-types">
              <span className="text-muted text-xs">Detected: </span>
              {recommendations.project_types.map((type, i) => (
                <span key={type} className="project-type-tag">
                  {type}
                  {i < recommendations.project_types.length - 1 ? ', ' : ''}
                </span>
              ))}
            </div>
          )}

          {/* Priority sections - critical expanded by default */}
          <div className="recommendation-sections">
            <PrioritySection
              priority="critical"
              recommendations={grouped.critical}
              defaultExpanded={true}
            />
            <PrioritySection
              priority="high"
              recommendations={grouped.high}
              defaultExpanded={summary.critical_missing === 0}
            />
            <PrioritySection priority="medium" recommendations={grouped.medium} />
            <PrioritySection priority="low" recommendations={grouped.low} />
          </div>
        </>
      )}
    </div>
  );
}

export default RecommendationsPanel;
