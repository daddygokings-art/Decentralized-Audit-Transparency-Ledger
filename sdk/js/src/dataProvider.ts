import { TokenBucket, RateLimitConfig, parseRateLimitHeaders } from './rateLimit';

export interface DataProviderRetryConfig {
  maxRetries?: number;
  initialBackoffMs?: number;
  maxBackoffMs?: number;
  backoffMultiplier?: number;
}

export interface DataProviderConfig {
  baseUrl: string;
  providerName?: string;
  apiKey?: string;
  timeoutMs?: number;
  headers?: Record<string, string>;
  rateLimit?: RateLimitConfig;
  retry?: DataProviderRetryConfig;
}

export interface RequestOptions {
  headers?: Record<string, string>;
  params?: Record<string, string | number | boolean | undefined>;
  timeoutMs?: number;
  signal?: AbortSignal;
}

// ============================================================================
// Standard Error Hierarchy
// ============================================================================

export class DataProviderError extends Error {
  readonly providerName: string;
  readonly statusCode?: number;
  readonly endpoint: string;
  readonly isRetryable: boolean;
  readonly responseBody?: unknown;

  constructor(
    message: string,
    options: {
      providerName: string;
      statusCode?: number;
      endpoint: string;
      isRetryable?: boolean;
      responseBody?: unknown;
      cause?: Error;
    }
  ) {
    super(message);
    this.name = 'DataProviderError';
    this.providerName = options.providerName;
    this.statusCode = options.statusCode;
    this.endpoint = options.endpoint;
    this.isRetryable = options.isRetryable ?? false;
    this.responseBody = options.responseBody;
    if (options.cause) {
      this.cause = options.cause;
    }
  }
}

export class RateLimitExceededError extends DataProviderError {
  readonly retryAfterSeconds?: number;

  constructor(providerName: string, endpoint: string, retryAfterSeconds?: number) {
    super(`Rate limit exceeded for provider "${providerName}" at ${endpoint}`, {
      providerName,
      statusCode: 429,
      endpoint,
      isRetryable: true,
    });
    this.name = 'RateLimitExceededError';
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

export class ProviderTimeoutError extends DataProviderError {
  constructor(providerName: string, endpoint: string, timeoutMs: number) {
    super(`Request to "${providerName}" timed out after ${timeoutMs}ms at ${endpoint}`, {
      providerName,
      statusCode: 408,
      endpoint,
      isRetryable: true,
    });
    this.name = 'ProviderTimeoutError';
  }
}

export class ProviderAuthenticationError extends DataProviderError {
  constructor(providerName: string, endpoint: string, statusCode = 401) {
    super(`Authentication failed for provider "${providerName}" at ${endpoint}`, {
      providerName,
      statusCode,
      endpoint,
      isRetryable: false,
    });
    this.name = 'ProviderAuthenticationError';
  }
}

// ============================================================================
// Base Data Provider Client
// ============================================================================

/**
 * Reusable abstract base client for external and internal data providers.
 * Consolidates HTTP client boilerplate, token bucket rate-limiting, exponential
 * backoff retries with jitter, and uniform error classification.
 */
export class BaseDataProviderClient {
  readonly baseUrl: string;
  readonly providerName: string;
  protected readonly apiKey?: string;
  protected readonly timeoutMs: number;
  protected readonly defaultHeaders: Record<string, string>;
  protected readonly rateLimiter?: TokenBucket;
  protected readonly retryConfig: Required<DataProviderRetryConfig>;

  constructor(config: DataProviderConfig) {
    this.baseUrl = config.baseUrl.replace(/\/+$/, '');
    this.providerName = config.providerName || 'DataProvider';
    this.apiKey = config.apiKey;
    this.timeoutMs = config.timeoutMs ?? 10000;
    this.defaultHeaders = {
      'Accept': 'application/json',
      'Content-Type': 'application/json',
      ...(config.headers || {}),
    };

    if (config.rateLimit) {
      this.rateLimiter = new TokenBucket(config.rateLimit);
    }

    this.retryConfig = {
      maxRetries: config.retry?.maxRetries ?? 3,
      initialBackoffMs: config.retry?.initialBackoffMs ?? 500,
      maxBackoffMs: config.retry?.maxBackoffMs ?? 10000,
      backoffMultiplier: config.retry?.backoffMultiplier ?? 2,
    };
  }

  /**
   * Hook for subclasses to supply dynamic headers (e.g., auth tokens, signatures).
   */
  protected async getHeaders(): Promise<Record<string, string>> {
    const headers = { ...this.defaultHeaders };
    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }
    return headers;
  }

  /**
   * Sends an HTTP request with automatic rate limiting, retries, and error mapping.
   */
  public async request<T = unknown>(
    method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH',
    endpoint: string,
    body?: unknown,
    options: RequestOptions = {}
  ): Promise<T> {
    const cleanEndpoint = endpoint.startsWith('/') ? endpoint : `/${endpoint}`;
    let url = `${this.baseUrl}${cleanEndpoint}`;

    if (options.params) {
      const searchParams = new URLSearchParams();
      for (const [key, val] of Object.entries(options.params)) {
        if (val !== undefined) {
          searchParams.append(key, String(val));
        }
      }
      const qs = searchParams.toString();
      if (qs) {
        url += (url.includes('?') ? '&' : '?') + qs;
      }
    }

    let attempt = 0;
    let lastError: Error | null = null;

    while (attempt <= this.retryConfig.maxRetries) {
      // 1. Rate Limiting enforcement
      if (this.rateLimiter) {
        await this.rateLimiter.acquire(1);
      }

      // 2. Abort & Timeout handling
      const timeoutMs = options.timeoutMs ?? this.timeoutMs;
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);

      // Connect caller's abort signal if supplied
      if (options.signal) {
        options.signal.addEventListener('abort', () => controller.abort());
      }

      try {
        const customHeaders = await this.getHeaders();
        const headers = {
          ...customHeaders,
          ...(options.headers || {}),
        };

        const fetchOptions: RequestInit = {
          method,
          headers,
          signal: controller.signal,
        };

        if (body !== undefined && method !== 'GET') {
          fetchOptions.body = typeof body === 'string' ? body : JSON.stringify(body);
        }

        const response = await fetch(url, fetchOptions);
        clearTimeout(timer);

        // Parse rate limit response headers if limiter is active
        if (this.rateLimiter) {
          const rlHeaders = parseRateLimitHeaders(response.headers);
          this.rateLimiter.applyHeaders(rlHeaders);
        }

        // 3. Handle Status Codes
        if (response.ok) {
          if (response.status === 204) {
            return undefined as unknown as T;
          }
          const contentType = response.headers.get('content-type') || '';
          if (contentType.includes('application/json')) {
            return (await response.json()) as T;
          }
          return (await response.text()) as unknown as T;
        }

        // Failure handling
        let responseBody: unknown;
        try {
          responseBody = await response.json();
        } catch {
          try {
            responseBody = await response.text();
          } catch {
            responseBody = null;
          }
        }

        const is429 = response.status === 429;
        const isServerErr = response.status >= 500 && response.status <= 504;

        if (response.status === 401 || response.status === 403) {
          throw new ProviderAuthenticationError(this.providerName, cleanEndpoint, response.status);
        }

        if (is429) {
          const rl = parseRateLimitHeaders(response.headers);
          throw new RateLimitExceededError(this.providerName, cleanEndpoint, rl.retryAfterSeconds);
        }

        const err = new DataProviderError(
          `Provider "${this.providerName}" returned HTTP ${response.status}: ${response.statusText}`,
          {
            providerName: this.providerName,
            statusCode: response.status,
            endpoint: cleanEndpoint,
            isRetryable: isServerErr,
            responseBody,
          }
        );

        if (!isServerErr) {
          throw err;
        }

        lastError = err;
      } catch (err: any) {
        clearTimeout(timer);

        if (err.name === 'AbortError') {
          lastError = new ProviderTimeoutError(this.providerName, cleanEndpoint, timeoutMs);
        } else if (err instanceof DataProviderError) {
          if (!err.isRetryable) {
            throw err;
          }
          lastError = err;
        } else {
          lastError = new DataProviderError(`Network error communicating with "${this.providerName}": ${err.message}`, {
            providerName: this.providerName,
            endpoint: cleanEndpoint,
            isRetryable: true,
            cause: err,
          });
        }

        if (attempt >= this.retryConfig.maxRetries) {
          break;
        }

        // Calculate exponential backoff with jitter
        const backoff = Math.min(
          this.retryConfig.initialBackoffMs * Math.pow(this.retryConfig.backoffMultiplier, attempt),
          this.retryConfig.maxBackoffMs
        );
        const jitter = Math.random() * 0.3 * backoff;
        await new Promise((res) => setTimeout(res, backoff + jitter));
        attempt++;
      }
    }

    throw lastError || new DataProviderError(`Failed to complete request to "${this.providerName}"`, {
      providerName: this.providerName,
      endpoint: cleanEndpoint,
    });
  }

  public get<T>(endpoint: string, params?: Record<string, string | number | boolean | undefined>, options?: RequestOptions): Promise<T> {
    return this.request<T>('GET', endpoint, undefined, { ...options, params });
  }

  public post<T>(endpoint: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return this.request<T>('POST', endpoint, body, options);
  }

  public put<T>(endpoint: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return this.request<T>('PUT', endpoint, body, options);
  }

  public delete<T>(endpoint: string, options?: RequestOptions): Promise<T> {
    return this.request<T>('DELETE', endpoint, undefined, options);
  }
}
