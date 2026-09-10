import type { ReactNode } from 'react';

interface InsightsSectionProps {
  title: string;
  icon: ReactNode;
  description?: string;
  /** True on a fresh install: show the empty text instead of a table of zeros. */
  isEmpty: boolean;
  emptyText: string;
  children: ReactNode;
}

/** Card shell shared by every Insights section, including its empty state. */
function InsightsSection({
  title,
  icon,
  description,
  isEmpty,
  emptyText,
  children,
}: InsightsSectionProps) {
  return (
    <section className="insights-section">
      <h3 className="section-title insights-section-title">
        {icon}
        {title}
      </h3>
      {description && <p className="insights-section-desc">{description}</p>}
      {isEmpty ? <p className="empty-state-small">{emptyText}</p> : children}
    </section>
  );
}

export default InsightsSection;
