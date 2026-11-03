import { NavLink } from 'react-router-dom';
import { Bell } from 'lucide-react';
import { formatCount, useNotificationSummary } from '../services/notificationStore';

/** Sidebar link to the inbox, with the unread count. Red only for failures. */
function NotificationBell() {
  const { unread, unreadFailures } = useNotificationSummary();
  const countClass = unreadFailures > 0 ? 'nav-count nav-count-failed' : 'nav-count';

  return (
    <NavLink
      to="/notifications"
      className="nav-link"
      aria-label={unread > 0 ? `Notifications, ${unread} unread` : 'Notifications'}
    >
      <Bell size={16} />
      <span>Notifications</span>
      {unread > 0 && <span className={countClass}>{formatCount(unread)}</span>}
    </NavLink>
  );
}

export default NotificationBell;
