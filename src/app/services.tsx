import { isTauri } from "@tauri-apps/api/core";
import { createContext, useContext, type ReactNode } from "react";
import { documentApi, type DocumentApi } from "../api/document";
import { meetingApi, type MeetingApi } from "../api/meeting";
import { secretApi, type SecretApi } from "../api/secret";
import { checkSidecarHealth, type SidecarStatus } from "../lib/sidecar";
import { ProviderRegistry } from "../providers/runtime";

export interface AppServices {
  documentApi: DocumentApi;
  meetingApi: MeetingApi;
  secretApi: SecretApi;
  checkSidecarHealth: () => Promise<SidecarStatus>;
  providers: ProviderRegistry;
  desktopRuntime: boolean;
}

export function createDefaultAppServices(): AppServices {
  return {
    documentApi,
    meetingApi,
    secretApi,
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
