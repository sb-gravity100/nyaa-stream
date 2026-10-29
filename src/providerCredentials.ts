import { useEffect, useState } from "preact/hooks";

export const PROVIDER_CREDENTIALS_STORAGE_KEY = "nyaa-stream:provider-credentials";

export interface ProviderCredentials {
  tmdbReadAccessToken: string;
  fanartApiKey: string;
}

type CredentialListener = (credentials: ProviderCredentials) => void;

const EMPTY_CREDENTIALS: ProviderCredentials = { tmdbReadAccessToken: "", fanartApiKey: "" };
const listeners = new Set<CredentialListener>();

function loadCredentials(): ProviderCredentials {
  console.debug("[artwork-settings] loading provider credential presence");
  try {
    const stored = localStorage.getItem(PROVIDER_CREDENTIALS_STORAGE_KEY);
    if (!stored) return EMPTY_CREDENTIALS;
    const parsed = JSON.parse(stored) as Partial<ProviderCredentials>;
    return {
      tmdbReadAccessToken: typeof parsed.tmdbReadAccessToken === "string" ? parsed.tmdbReadAccessToken : "",
      fanartApiKey: typeof parsed.fanartApiKey === "string" ? parsed.fanartApiKey : "",
    };
  } catch (error) {
    console.error("[artwork-settings] failed to read provider credentials", { error: String(error) });
    return EMPTY_CREDENTIALS;
  }
}

let currentCredentials = loadCredentials();

export function getProviderCredentials(): ProviderCredentials {
  console.debug("[artwork-settings] read credentials", { hasTmdb: !!currentCredentials.tmdbReadAccessToken, hasFanart: !!currentCredentials.fanartApiKey });
  return currentCredentials;
}

export function saveProviderCredentials(credentials: ProviderCredentials): void {
  console.info("[artwork-settings] save credentials requested", { hasTmdb: !!credentials.tmdbReadAccessToken, hasFanart: !!credentials.fanartApiKey });
  currentCredentials = {
    tmdbReadAccessToken: credentials.tmdbReadAccessToken.trim(),
    fanartApiKey: credentials.fanartApiKey.trim(),
  };
  try {
    localStorage.setItem(PROVIDER_CREDENTIALS_STORAGE_KEY, JSON.stringify(currentCredentials));
  } catch (error) {
    console.error("[artwork-settings] failed to save provider credentials", { error: String(error) });
  }
  for (const listener of listeners) listener(currentCredentials);
}

export function subscribeProviderCredentials(listener: CredentialListener): () => void {
  console.debug("[artwork-settings] subscribe", { listeners: listeners.size + 1 });
  listeners.add(listener);
  return () => {
    console.debug("[artwork-settings] unsubscribe", { listeners: listeners.size - 1 });
    listeners.delete(listener);
  };
}

export function useProviderCredentials(): ProviderCredentials {
  const [credentials, setCredentials] = useState(currentCredentials);
  useEffect(() => {
    console.debug("[artwork-settings] subscribe hook");
    listeners.add(setCredentials);
    setCredentials(currentCredentials);
    return () => {
      console.debug("[artwork-settings] unsubscribe hook");
      listeners.delete(setCredentials);
    };
  }, []);
  return credentials;
}
