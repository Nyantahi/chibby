import { useEffect, useState } from 'react';
import { nextRunTimes } from '../services/api';

export interface NextRuns {
  /** Upcoming fire times as RFC3339 strings. Empty while loading or on error. */
  times: string[];
  /** Backend parse error for the cron expression, shown inline next to the field. */
  error: string | null;
}

const EMPTY: NextRuns = { times: [], error: null };

/**
 * Ask the backend for the next `count` fire times of a cron expression.
 *
 * The backend cron parser is the only thing that knows whether an expression is
 * valid (5-field and 6-field forms both parse), so the preview doubles as the
 * validator — an unparseable cron comes back as `error` for inline display.
 */
export function useNextRuns(cron: string, count = 5): NextRuns {
  const [state, setState] = useState<NextRuns>(EMPTY);
  const trimmed = cron.trim();

  useEffect(() => {
    if (!trimmed) return;
    let ignore = false;
    nextRunTimes(trimmed, count)
      .then((times) => {
        if (!ignore) setState({ times, error: null });
      })
      .catch((err) => {
        if (!ignore) setState({ times: [], error: String(err) });
      });
    return () => {
      ignore = true;
    };
  }, [trimmed, count]);

  return trimmed ? state : EMPTY;
}
