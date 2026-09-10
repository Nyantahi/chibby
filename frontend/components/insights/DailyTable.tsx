import { CalendarDays } from 'lucide-react';
import InsightsSection from './InsightsSection';
import { formatDay } from '../../utils/insights';
import type { DailyCount } from '../../types';

interface DailyTableProps {
  daily: DailyCount[];
}

/** A row per day, oldest first. Deliberately a table and not a chart. */
function DailyTable({ daily }: DailyTableProps) {
  const hasRuns = daily.some((day) => day.runs > 0);

  return (
    <InsightsSection
      title="Daily"
      icon={<CalendarDays size={16} />}
      description="One row per day in the window, oldest first."
      isEmpty={!hasRuns}
      emptyText="No runs on any day in this window."
    >
      <div className="insights-table-wrap">
        <table className="insights-table insights-table-compact">
          <thead>
            <tr>
              <th>Date</th>
              <th>Runs</th>
              <th>Ok</th>
              <th>Failed</th>
            </tr>
          </thead>
          <tbody>
            {daily.map((day) => (
              <tr key={day.date} className={day.runs === 0 ? 'insights-row-quiet' : undefined}>
                <td>{formatDay(day.date)}</td>
                <td>{day.runs}</td>
                <td>{day.succeeded}</td>
                <td className={day.failed > 0 ? 'insights-cell-failed' : undefined}>
                  {day.failed}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </InsightsSection>
  );
}

export default DailyTable;
