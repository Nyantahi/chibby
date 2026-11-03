import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AlertTriangle, Bell, CheckCheck, CheckCircle2, Info, Trash2, XCircle } from 'lucide-react';
import { listProjects } from '../services/api';
import { notificationLink } from '../services/notificationLinks';
import {
  readAllNotifications,
  readNotification,
  reloadNotifications,
  removeNotification,
  useNotifications,
  useNotificationSummary,
} from '../services/notificationStore';
import type { NotificationEntry, NotificationSeverity, ProjectInfo } from '../types';

const SEVERITY_ICONS: Record<NotificationSeverity, typeof Info> = {
  success: CheckCircle2,
  failure: XCircle,
  info: Info,
  attention: AlertTriangle,
};

/** The inbox: everything Chibby has reported, newest first. */
function Notifications() {
  const navigate = useNavigate();
  const entries = useNotifications();
  const { unread } = useNotificationSummary();
  const [projects, setProjects] = useState<ProjectInfo[]>([]);

  useEffect(() => {
    void reloadNotifications();
    Promise.resolve(listProjects())
      .then((list) => setProjects(list ?? []))
      .catch(() => {});
  }, []);

  function handleOpen(entry: NotificationEntry) {
    if (!entry.read_at) void readNotification(entry.id);
    const link = notificationLink(entry, projects);
    if (link) navigate(link.to, { state: link.state });
  }

  return (
    <div className="page-content">
      <div className="page-header">
        <h2 className="page-title">Notifications</h2>
        <button
          className="btn btn-sm btn-ghost"
          onClick={() => void readAllNotifications()}
          disabled={unread === 0}
        >
          <CheckCheck size={14} /> Mark all read
        </button>
      </div>

      {entries.length === 0 ? (
        <div className="empty-state">
          <Bell size={24} />
          <h3>No notifications</h3>
          <p>Finished runs show up here, including ones from schedules and the CLI.</p>
        </div>
      ) : (
        <ul className="notification-list">
          {entries.map((entry) => {
            const Icon = SEVERITY_ICONS[entry.severity];
            const rowClass = entry.read_at
              ? 'notification-row'
              : 'notification-row notification-row-unread';
            return (
              <li key={entry.id} className={rowClass}>
                <button className="notification-open" onClick={() => handleOpen(entry)}>
                  <Icon size={16} className={`notification-icon-${entry.severity}`} />
                  <span className="notification-text">
                    <span className="notification-title">{entry.title}</span>
                    <span className="notification-body">{entry.body}</span>
                    <time className="notification-time" dateTime={entry.created_at}>
                      {new Date(entry.created_at).toLocaleString()}
                    </time>
                  </span>
                </button>
                <button
                  className="btn btn-xs btn-danger-icon"
                  aria-label={`Delete notification: ${entry.title}`}
                  onClick={() => void removeNotification(entry.id)}
                >
                  <Trash2 size={12} />
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export default Notifications;
