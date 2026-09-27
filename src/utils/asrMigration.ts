import type { AsrCredentials } from "../types";

/** A legacy browser cache must never replace existing backend credentials. */
export function hasBackendAsrCredentials(credentials: Partial<AsrCredentials> | undefined): boolean {
  return Boolean(credentials?.qwen_api_key?.trim() || credentials?.sensevoice_api_key?.trim()
    || credentials?.doubao_app_id?.trim() || credentials?.doubao_access_token?.trim()
    || credentials?.doubao_ime_device_id?.trim() || credentials?.doubao_ime_token?.trim()
    || credentials?.doubao_ime_cdid?.trim());
}
