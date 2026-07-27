import { invoke } from "@tauri-apps/api/core";

export interface SidecarHealth {
  status: "ok";
  service: "meetinglens-sidecar";
  version: string;
  api_version: "v1";
}

export function checkSidecarHealth(): Promise<SidecarHealth> {
  return invoke<SidecarHealth>("check_sidecar_health");
}
