import { NovaClient, NovaError, Errors } from '../src';

describe('NovaClient', () => {
  it('should create a client with default config', () => {
    const client = new NovaClient({ auth: { type: 'none' } });
    expect(client).toBeInstanceOf(NovaClient);
    expect(client.runtime).toBeDefined();
    expect(client.db).toBeDefined();
    expect(client.cache).toBeDefined();
    expect(client.queue).toBeDefined();
    expect(client.scheduler).toBeDefined();
    expect(client.search).toBeDefined();
    expect(client.blob).toBeDefined();
    expect(client.auth).toBeDefined();
  });

  it('should create a client with token auth', () => {
    const client = new NovaClient({ auth: { type: 'token', token: 'test-token' } });
    expect(client).toBeInstanceOf(NovaClient);
  });

  it('should create a client with api key auth', () => {
    const client = new NovaClient({ auth: { type: 'api-key', apiKey: 'test-key' } });
    expect(client).toBeInstanceOf(NovaClient);
  });

  it('should create client via factory function', () => {
    const client = new NovaClient({ auth: { type: 'none' } });
    expect(client).toBeInstanceOf(NovaClient);
  });
});

describe('NovaError', () => {
  it('should create a basic error', () => {
    const error = new NovaError({
      code: 'NOT_FOUND',
      message: 'Resource not found',
      httpStatus: 404,
    });
    expect(error.code).toBe('NOT_FOUND');
    expect(error.message).toBe('Resource not found');
    expect(error.httpStatus).toBe(404);
    expect(error.retryable).toBe(false);
  });

  it('should create a retryable error', () => {
    const error = new NovaError({
      code: 'INTERNAL_ERROR',
      message: 'Server error',
      httpStatus: 500,
      retryable: true,
    });
    expect(error.isRetryable()).toBe(true);
  });

  it('should format summary correctly', () => {
    const error = new NovaError({
      code: 'NOT_FOUND',
      message: 'User not found',
      requestId: 'req-123',
    });
    expect(error.toSummary()).toBe('[NOT_FOUND] User not found (req: req-123)');
  });

  it('should get retry delay from extensions', () => {
    const error = new NovaError({
      code: 'RATE_LIMITED',
      message: 'Too many requests',
      extensions: { retryAfterMs: 5000 },
    });
    expect(error.retryAfterMs()).toBe(5000);
  });
});

describe('Errors helper', () => {
  it('should create not found error', () => {
    const error = Errors.notFound('User not found');
    expect(error.code).toBe('NOT_FOUND');
    expect(error.httpStatus).toBe(404);
  });

  it('should create unauthorized error', () => {
    const error = Errors.unauthorized();
    expect(error.code).toBe('UNAUTHENTICATED');
    expect(error.httpStatus).toBe(401);
  });

  it('should create token expired error', () => {
    const error = Errors.tokenExpired();
    expect(error.code).toBe('TOKEN_EXPIRED');
    expect(error.retryable).toBe(true);
  });

  it('should create rate limited error', () => {
    const error = Errors.rateLimited(5000);
    expect(error.code).toBe('RATE_LIMITED');
    expect(error.extensions?.retryAfterMs).toBe(5000);
  });

  it('should create timeout error', () => {
    const error = Errors.timeout();
    expect(error.retryable).toBe(true);
  });

  it('should create connection error', () => {
    const error = Errors.connectionError('ECONNREFUSED');
    expect(error.code).toBe('CONNECTION_ERROR');
    expect(error.message).toContain('ECONNREFUSED');
  });

  it('should create error from HTTP status', () => {
    const error = Errors.fromHttpStatus(404, { error: { message: 'Not found' } });
    expect(error.code).toBe('NOT_FOUND');
    expect(error.httpStatus).toBe(404);
  });

  it('should mark 5xx as retryable', () => {
    const error = Errors.fromHttpStatus(500, { message: 'Error' });
    expect(error.retryable).toBe(true);
  });

  it('should mark 429 as retryable', () => {
    const error = Errors.fromHttpStatus(429, { message: 'Rate limited' });
    expect(error.retryable).toBe(true);
  });
});
