import type { HttpClient } from './client';
import type { AuthResult, User, ApiKey, ApiKeyFull, Role, Connection, PaginationInput } from './types';

export class AuthClient {
  constructor(
    private http: HttpClient
  ) {}

  async login(username: string, password: string): Promise<AuthResult> {
    const response = await this.http.post<AuthResult>('/auth/login', { username, password });
    return response.data;
  }

  async register(input: {
    username: string;
    email: string;
    password: string;
    displayName: string;
  }): Promise<AuthResult> {
    const response = await this.http.post<AuthResult>('/auth/register', input);
    return response.data;
  }

  async refreshToken(refreshToken: string): Promise<AuthResult> {
    const response = await this.http.post<AuthResult>('/auth/token/refresh', { refreshToken });
    return response.data;
  }

  async logout(): Promise<void> {
    await this.http.post('/auth/logout');
  }

  async me(): Promise<User> {
    const response = await this.http.get<User>('/auth/me');
    return response.data;
  }

  async listUsers(options?: {
    status?: string;
    role?: string;
    search?: string;
    pagination?: PaginationInput;
  }): Promise<Connection<User>> {
    const response = await this.http.get<Connection<User>>('/auth/users', { query: options as Record<string, unknown> });
    return response.data;
  }

  async getUser(id: string): Promise<User> {
    const response = await this.http.get<User>(`/auth/users/${id}`);
    return response.data;
  }

  async createUser(input: {
    username: string;
    email: string;
    password?: string;
    displayName?: string;
    roles?: string[];
  }): Promise<User> {
    const response = await this.http.post<User>('/auth/users', input);
    return response.data;
  }

  async updateUser(id: string, input: {
    displayName?: string;
    email?: string;
    metadata?: Record<string, unknown>;
  }): Promise<User> {
    const response = await this.http.patch<User>(`/auth/users/${id}`, input);
    return response.data;
  }

  async deleteUser(id: string): Promise<void> {
    await this.http.delete(`/auth/users/${id}`);
  }

  async suspendUser(id: string, reason?: string): Promise<User> {
    const response = await this.http.post<User>(`/auth/users/${id}/suspend`, { reason });
    return response.data;
  }

  async activateUser(id: string): Promise<User> {
    const response = await this.http.post<User>(`/auth/users/${id}/activate`);
    return response.data;
  }

  async listApiKeys(options?: PaginationInput): Promise<Connection<ApiKey>> {
    const response = await this.http.get<Connection<ApiKey>>('/auth/keys', { query: options as Record<string, unknown> });
    return response.data;
  }

  async createApiKey(input: {
    name: string;
    permissions?: string[];
    roles?: string[];
    expiresAt?: Date;
  }): Promise<ApiKeyFull> {
    const response = await this.http.post<ApiKeyFull>('/auth/keys', input);
    return response.data;
  }

  async deleteApiKey(id: string): Promise<void> {
    await this.http.delete(`/auth/keys/${id}`);
  }

  async listRoles(): Promise<Role[]> {
    const response = await this.http.get<Role[]>('/auth/roles');
    return response.data;
  }

  async createRole(input: { name: string; description: string; permissions: string[] }): Promise<Role> {
    const response = await this.http.post<Role>('/auth/roles', input);
    return response.data;
  }

  async deleteRole(name: string): Promise<void> {
    await this.http.delete(`/auth/roles/${encodeURIComponent(name)}`);
  }

  async grantRole(userId: string, roleName: string): Promise<User> {
    const response = await this.http.post<User>(`/auth/users/${userId}/roles`, { role: roleName });
    return response.data;
  }

  async revokeRole(userId: string, roleName: string): Promise<User> {
    const response = await this.http.delete<User>(`/auth/users/${userId}/roles/${encodeURIComponent(roleName)}`);
    return response.data;
  }
}
