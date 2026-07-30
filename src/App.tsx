import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import {
  AppLink,
  NavigationProvider,
  useNavigation,
  type AppRoute,
} from "./app/navigation";
import {
  defaultAppServices,
  ServicesProvider,
  type AppServices,
} from "./app/services";
import { StatusBadge } from "./components/StatusBadge";
import { HistoryPage } from "./pages/HistoryPage";
import { HomePage, type DatabaseStatus } from "./pages/HomePage";
import { LiveMeetingPage } from "./pages/LiveMeetingPage";
import { MeetingDetailPage } from "./pages/MeetingDetailPage";
import { NewMeetingPage } from "./pages/NewMeetingPage";
import { SettingsPage, type LocalServiceStatus } from "./pages/SettingsPage";

export interface AppProps {
  services?: AppServices;
}

function App({ services = defaultAppServices }: AppProps) {
  return (
    <ServicesProvider value={services}>
      <NavigationProvider>
        <MeetingLensApp services={services} />
      </NavigationProvider>
    </ServicesProvider>
  );
}

function MeetingLensApp({ services }: { services: AppServices }) {
  const { route } = useNavigation();
  const [databaseStatus, setDatabaseStatus] =
    useState<DatabaseStatus>("checking");
  const [localServiceStatus, setLocalServiceStatus] =
    useState<LocalServiceStatus>(
      services.desktopRuntime ? "checking" : "desktop-only",
    );
  const [meetingStatus, setMeetingStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!services.desktopRuntime) return;
    let active = true;
    void services
      .checkSidecarHealth()
      .then(() => {
        if (active) setLocalServiceStatus("healthy");
      })
      .catch(() => {
        if (active) setLocalServiceStatus("unavailable");
      });
    return () => {
      active = false;
    };
  }, [services]);

  const updateDatabaseStatus = useCallback((status: DatabaseStatus) => {
    setDatabaseStatus(status);
  }, []);
  const updateMeetingStatus = useCallback((status: string | null) => {
    setMeetingStatus(status);
  }, []);

  const pageTitle = useMemo(() => titleForRoute(route), [route]);

  return (
    <div className="desktop-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <aside className="sidebar">
        <AppLink aria-label="MeetingLens home" className="brand" to="/">
          <span className="brand-mark" aria-hidden="true">
            ML
          </span>
          <span>
            <strong>MeetingLens</strong>
            <small>Local meeting workspace</small>
          </span>
        </AppLink>
        <nav aria-label="Primary navigation">
          <NavItem
            active={route.name === "home"}
            label="Home"
            mark="HM"
            to="/"
          />
          <NavItem
            active={route.name === "new"}
            label="New meeting"
            mark="NM"
            to="/new"
          />
          <NavItem
            active={
              route.name === "history" ||
              route.name === "meeting" ||
              route.name === "live"
            }
            label="History"
            mark="HS"
            to="/history"
          />
          <NavItem
            active={route.name === "settings"}
            label="Settings"
            mark="ST"
            to="/settings"
          />
        </nav>
        <div className="privacy-note">
          <span aria-hidden="true">LP</span>
          <div>
            <strong>Local-first</strong>
            <p>No recording or external AI in this demo.</p>
          </div>
        </div>
      </aside>

      <div className="workspace">
        <header className="workspace-header">
          <div>
            <p className="breadcrumb">MeetingLens / {pageTitle}</p>
            <h2>{pageTitle}</h2>
          </div>
          <div className="runtime-status" aria-label="Runtime status">
            <StatusChip
              label="Service"
              status={
                localServiceStatus === "healthy"
                  ? "Healthy"
                  : localServiceStatus === "checking"
                    ? "Checking"
                    : "Unavailable"
              }
              tone={
                localServiceStatus === "healthy"
                  ? "success"
                  : localServiceStatus === "checking"
                    ? "neutral"
                    : "warning"
              }
            />
            <StatusChip
              label="Database"
              status={
                databaseStatus === "ready"
                  ? "Ready"
                  : databaseStatus === "checking"
                    ? "Checking"
                    : "Unavailable"
              }
              tone={
                databaseStatus === "ready"
                  ? "success"
                  : databaseStatus === "checking"
                    ? "neutral"
                    : "warning"
              }
            />
            {(route.name === "meeting" || route.name === "live") &&
            meetingStatus ? (
              <StatusBadge status={meetingStatus} />
            ) : null}
          </div>
        </header>

        <main id="main-content">
          <RouteContent
            databaseStatus={databaseStatus}
            localServiceStatus={localServiceStatus}
            onDatabaseStatus={updateDatabaseStatus}
            onMeetingStatus={updateMeetingStatus}
            route={route}
          />
        </main>
      </div>
    </div>
  );
}

function RouteContent({
  databaseStatus,
  localServiceStatus,
  onDatabaseStatus,
  onMeetingStatus,
  route,
}: {
  databaseStatus: DatabaseStatus;
  localServiceStatus: LocalServiceStatus;
  onDatabaseStatus: (status: DatabaseStatus) => void;
  onMeetingStatus: (status: string | null) => void;
  route: AppRoute;
}) {
  switch (route.name) {
    case "home":
      return <HomePage onDatabaseStatus={onDatabaseStatus} />;
    case "new":
      return <NewMeetingPage />;
    case "history":
      return <HistoryPage onDatabaseStatus={onDatabaseStatus} />;
    case "meeting":
      return (
        <MeetingDetailPage
          meetingId={route.meetingId}
          onDatabaseStatus={onDatabaseStatus}
          onMeetingStatus={onMeetingStatus}
        />
      );
    case "live":
      return (
        <LiveMeetingPage
          meetingId={route.meetingId}
          onDatabaseStatus={onDatabaseStatus}
          onMeetingStatus={onMeetingStatus}
        />
      );
    case "settings":
      return (
        <SettingsPage
          databaseStatus={databaseStatus}
          localServiceStatus={localServiceStatus}
        />
      );
  }
}

function NavItem({
  active,
  label,
  mark,
  to,
}: {
  active: boolean;
  label: string;
  mark: string;
  to: string;
}) {
  return (
    <AppLink
      aria-current={active ? "page" : undefined}
      className={active ? "nav-item nav-item--active" : "nav-item"}
      to={to}
    >
      <span aria-hidden="true">{mark}</span>
      {label}
    </AppLink>
  );
}

function StatusChip({
  label,
  status,
  tone,
}: {
  label: string;
  status: string;
  tone: "success" | "warning" | "neutral";
}) {
  return (
    <span className={`runtime-chip runtime-chip--${tone}`}>
      <span className="runtime-chip__dot" aria-hidden="true" />
      <span>
        <small>{label}</small>
        <strong>{status}</strong>
      </span>
    </span>
  );
}

function titleForRoute(route: AppRoute): string {
  switch (route.name) {
    case "home":
      return "Home";
    case "new":
      return "New meeting";
    case "history":
      return "Meeting history";
    case "meeting":
      return "Meeting details";
    case "live":
      return "Meeting in progress";
    case "settings":
      return "Settings";
  }
}

export default App;
