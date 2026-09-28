import type {
  AppConfig,
  SearchConfig,
  SearchProviderConfig,
} from "../types";

type WebSearchRuntimeConfig = Pick<AppConfig, "assistant_config" | "search_config">;

function hasValue(value: string | null | undefined): boolean {
  return Boolean(value?.trim());
}

export function isSearchProviderApiConfigured(provider: SearchProviderConfig): boolean {
  if (provider.provider_type === "searxng") {
    return hasValue(provider.endpoint);
  }

  return hasValue(provider.api_key);
}

export function isSearchProviderRuntimeUsable(provider: SearchProviderConfig): boolean {
  return provider.enabled && isSearchProviderApiConfigured(provider);
}

export function resolveSearchDefaultProviderId(
  providers: SearchProviderConfig[],
  defaultProviderId: string | null,
): string | null {
  const currentProvider = defaultProviderId
    ? providers.find((provider) => provider.id === defaultProviderId)
    : undefined;

  if (currentProvider && isSearchProviderRuntimeUsable(currentProvider)) {
    return currentProvider.id;
  }

  const firstUsableProvider = providers.find(isSearchProviderRuntimeUsable);
  if (firstUsableProvider) return firstUsableProvider.id;

  if (currentProvider && isSearchProviderApiConfigured(currentProvider)) {
    return currentProvider.id;
  }

  const firstConfiguredProvider = providers.find(isSearchProviderApiConfigured);
  if (firstConfiguredProvider) return firstConfiguredProvider.id;

  return null;
}

export function selectSearchDefaultProvider(
  config: SearchConfig,
  providerId: string,
): SearchConfig {
  const selectedProvider = config.providers.find((provider) => provider.id === providerId);
  if (!selectedProvider || !isSearchProviderApiConfigured(selectedProvider)) return config;

  const providers = selectedProvider.enabled
    ? config.providers
    : config.providers.map((provider) =>
        provider.id === providerId && !provider.enabled
          ? { ...provider, enabled: true }
          : provider,
      );

  if ((config.default_provider_id ?? null) === providerId && providers === config.providers) {
    return config;
  }

  return {
    ...config,
    providers,
    default_provider_id: providerId,
  };
}

export function isDefaultSearchProviderRuntimeUsable(config: SearchConfig): boolean {
  const defaultProviderId = config.default_provider_id?.trim();
  if (!defaultProviderId) return false;

  const defaultProvider = config.providers.find((provider) => provider.id === defaultProviderId);
  return Boolean(defaultProvider && isSearchProviderRuntimeUsable(defaultProvider));
}

export function hasSearchRuntimeUsableProvider(config: SearchConfig): boolean {
  return config.providers.some(isSearchProviderRuntimeUsable);
}

export function resolveInitialWebSearchEnabled(config: WebSearchRuntimeConfig): boolean {
  return Boolean(
    config.search_config && hasSearchRuntimeUsableProvider(config.search_config),
  );
}
