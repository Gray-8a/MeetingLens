import { isTauri } from "@tauri-apps/api/core";
import { createContext, useContext, type ReactNode } from "react";
import { meetingApi, type MeetingApi } from "../api/meeting";
import { checkSidecarHealth, type SidecarStatus } from "../lib/sidecar";
import { ProviderRegistry } from "../providers/runtime";

export interface AppServices {
  meetingApi: MeetingApi;
  checkSidecarHealth: () => Promise<SidecarStatus>;
  providers: ProviderRegistry;
  desktopRuntime: boolean;
}

export function createDefaultAppServices(): AppServices {
  return {
    meetingApi,
    checkSidecarHealth,
    providers: new ProviderRegistry(),
    desktopRuntime: isTauri(),
  };
}

const ServicesContext = createContext<AppServices | null>(null);

export function ServicesProvider({
  children,
  value,
}: {
  children: ReactNode;
  value: AppServices;
}) {
  return (
    <ServicesContext.Provider value={value}>
      {children}
    </ServicesContext.Provider>
  );
}

export function useServices(): AppServices {
  const services = useContext(ServicesContext);
  if (services === null) {
    throw new Error("App services are unavailable.");
  }
  return services;
}
