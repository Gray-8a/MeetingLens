import {
  useCallback,
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type AnchorHTMLAttributes,
  type MouseEvent,
  type ReactNode,
} from "react";

export type AppRoute =
  | { name: "home" }
  | { name: "summary" }
  | { name: "new" }
  | { name: "history" }
  | { name: "meeting"; meetingId: string }
  | { name: "live"; meetingId: string }
  | { name: "transcript"; meetingId: string }
  | { name: "minutes"; meetingId: string; summaryId: string | null }
  | { name: "settings" }
  | { name: "help" };

interface NavigationValue {
  route: AppRoute;
  navigate: (path: string) => void;
  registerBlocker: (blocker: NavigationBlocker) => () => void;
}

interface NavigationBlocker {
  shouldBlock: () => boolean;
  onBlocked: (path: string) => void;
}

const NavigationContext = createContext<NavigationValue | null>(null);

function currentPath(): string {
  const path = window.location.hash.replace(/^#/, "");
  return path.startsWith("/") ? path : "/";
}

export function parseRoute(path: string): AppRoute {
  if (path === "/summary") return { name: "summary" };
  if (path === "/new") return { name: "new" };
  if (path === "/history") return { name: "history" };
  if (path === "/settings") return { name: "settings" };
  if (path === "/help") return { name: "help" };

  const liveMatch = path.match(/^\/meetings\/([^/]+)\/live$/);
  if (liveMatch) {
    return { name: "live", meetingId: safeDecode(liveMatch[1]) };
  }

  const transcriptMatch = path.match(/^\/meetings\/([^/]+)\/transcript$/);
  if (transcriptMatch) {
    return {
      name: "transcript",
      meetingId: safeDecode(transcriptMatch[1]),
    };
  }

  const minutesMatch = path.match(
    /^\/meetings\/([^/]+)\/minutes(?:\/([^/]+))?$/,
  );
  if (minutesMatch) {
    return {
      name: "minutes",
      meetingId: safeDecode(minutesMatch[1]),
      summaryId: minutesMatch[2] ? safeDecode(minutesMatch[2]) : null,
    };
  }

  const meetingMatch = path.match(/^\/meetings\/([^/]+)$/);
  if (meetingMatch) {
    return { name: "meeting", meetingId: safeDecode(meetingMatch[1]) };
  }

  return { name: "home" };
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function meetingPath(id: string): string {
  return `/meetings/${encodeURIComponent(id)}`;
}

export function liveMeetingPath(id: string): string {
  return `${meetingPath(id)}/live`;
}

export function NavigationProvider({ children }: { children: ReactNode }) {
  const [path, setPath] = useState(currentPath);
  const pathRef = useRef(path);
  const blockerRef = useRef<NavigationBlocker | null>(null);

  useEffect(() => {
    const syncPath = () => {
      const nextPath = currentPath();
      const current = pathRef.current;
      if (nextPath === current) return;
      const blocker = blockerRef.current;
      if (blocker?.shouldBlock()) {
        window.history.replaceState(null, "", `#${current}`);
        blocker.onBlocked(nextPath);
        return;
      }
      pathRef.current = nextPath;
      setPath(nextPath);
    };
    window.addEventListener("hashchange", syncPath);
    window.addEventListener("popstate", syncPath);
    return () => {
      window.removeEventListener("hashchange", syncPath);
      window.removeEventListener("popstate", syncPath);
    };
  }, []);

  const navigate = useCallback((nextPath: string) => {
    const normalized = nextPath.startsWith("/") ? nextPath : "/";
    if (normalized === pathRef.current) return;
    const blocker = blockerRef.current;
    if (blocker?.shouldBlock()) {
      blocker.onBlocked(normalized);
      return;
    }
    window.history.pushState(null, "", `#${normalized}`);
    pathRef.current = normalized;
    setPath(normalized);
  }, []);

  const registerBlocker = useCallback((blocker: NavigationBlocker) => {
    blockerRef.current = blocker;
    return () => {
      if (blockerRef.current === blocker) blockerRef.current = null;
    };
  }, []);

  const value = useMemo<NavigationValue>(
    () => ({
      route: parseRoute(path),
      navigate,
      registerBlocker,
    }),
    [navigate, path, registerBlocker],
  );

  return (
    <NavigationContext.Provider value={value}>
      {children}
    </NavigationContext.Provider>
  );
}

export function useNavigation(): NavigationValue {
  const value = useContext(NavigationContext);
  if (value === null) {
    throw new Error("Navigation is unavailable.");
  }
  return value;
}

export function AppLink({
  to,
  onClick,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { to: string }) {
  const { navigate } = useNavigation();

  function handleClick(event: MouseEvent<HTMLAnchorElement>) {
    onClick?.(event);
    if (!event.defaultPrevented) {
      event.preventDefault();
      navigate(to);
    }
  }

  return <a {...props} href={`#${to}`} onClick={handleClick} />;
}
