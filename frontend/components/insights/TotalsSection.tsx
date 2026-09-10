import { Gauge } from 'lucide-react';
import DeltaBadge from './DeltaBadge';
import InsightsSection from './InsightsSection';
import { formatMs, formatRate, NO_VALUE } from '../../utils/insights';
import type { TrendComparison } from '../../types';

interface TotalsSectionProps {
  totals: TrendComparison;
  windowDays: number;
}

/** Headline numbers for the window, each against the window before it. */
function TotalsSection({ totals, windowDays }: TotalsSectionProps) {
  const { current, previous } = totals;
  const hasRuns = current.runs > 0;

  return (
    <InsightsSection
      title="Totals"
      icon={<Gauge size={16} />}
      description={`Last ${windowDays} days, compared with the ${windowDays} days before.`}
      isEmpty={current.runs === 0 && previous.runs === 0}
      emptyText="No runs recorded in either window yet."
    >
      <div className="insights-stats">
        <div className="stat-card insights-stat">
          <div className="stat-value">{current.runs}</div>
          <div className="stat-label">Runs</div>
          <div className="insights-stat-sub">{previous.runs} in the previous window</div>
        </div>

        <div className="stat-card insights-stat">
          <div className="stat-value">{hasRuns ? formatRate(current.success_rate) : NO_VALUE}</div>
          <div className="stat-label">Success rate</div>
          <div className="insights-stat-sub">
            <DeltaBadge value={totals.success_rate_delta} unit="points" />
          </div>
        </div>

        <div className="stat-card insights-stat">
          <div className="stat-value insights-outcomes">
            <span className="insights-outcome-ok">{current.succeeded}</span>
            <span className="insights-outcome-sep">/</span>
            <span className="insights-outcome-failed">{current.failed}</span>
            <span className="insights-outcome-sep">/</span>
            <span className="insights-outcome-cancelled">{current.cancelled}</span>
          </div>
          <div className="stat-label">Ok / failed / cancelled</div>
          <div className="insights-stat-sub">Cancelled runs are kept out of the rate</div>
        </div>

        <div className="stat-card insights-stat">
          <div className="stat-value">{formatMs(current.avg_duration_ms)}</div>
          <div className="stat-label">Avg duration</div>
          <div className="insights-stat-sub">
            <DeltaBadge value={totals.avg_duration_delta_pct} unit="percent" lowerIsBetter />
          </div>
        </div>

        <div className="stat-card insights-stat">
          <div className="stat-value">{formatMs(current.p95_duration_ms)}</div>
          <div className="stat-label">p95 duration</div>
          <div className="insights-stat-sub">
            {formatMs(previous.p95_duration_ms)} in the previous window
          </div>
        </div>

        <div className="stat-card insights-stat">
          <div className={`stat-value ${current.unattended_failures > 0 ? 'stat-failed' : ''}`}>
            {current.unattended_runs}
          </div>
          <div className="stat-label">Unattended runs</div>
          <div className="insights-stat-sub">
            {current.unattended_failures} failed with nobody watching
          </div>
        </div>
      </div>
    </InsightsSection>
  );
}

export default TotalsSection;
