import type { HttpClient } from './client';
import type { AuthResult, User, ApiKey, PaginationInput } from './types';

export class AuthClient {
  constructor(
    private http: HttpClient
  ) {}

  async login(username: string, password: string): Promise<AuthResult> {
    const response = await this.http.post<AuthResult>('/auth/login', { username, password });
    return response.data;
  }

  async refreshToken(refreshToken: string): Promise<AuthResult> {
    const response = await this.http.post<AuthResult>('/auth/refresh', { refresh_token: refreshToken });
    return response.data;
  }

  async logout(): Promise<void> {
    await this.http.post('/auth/logout');
  }

  async listUsers(options?: PaginationInput): Promise<{ data: User[]; pagination: { cursor: string | null; limit: number; has_more: boolean } }> {
    const response = await this.http.get<{ data: User[]; pagination: { cursor: string | null; limit: number; has_more: boolean } }>('/auth/users', { query: options as Record<string, unknown> });
    return response.data;
  }

  async getUser(id: string): Promise<User> {
    const response = await this.http.get<User>(`/auth/users/${id}`);
    return response.data;
  }

  async createUser(input: {
    username: string;
    password: string;
    roles?: string[];
  }): Promise<{ id: string; username: string; roles: string[]; status: string }> {
    const response = await this.http.post<{ id: string; username: string; roles: string[]; status: string }>('/auth/users', input);
    return response.data;
  }

  async deleteUser(id: string): Promise<{ status: string; id: string }> {
    const response = await this.http.delete<{ status: string; id: string }>(`/auth/users/${id}`);
    return response.data;
  }

  async updateRoles(id: string, roles: string[]): Promise<{ status: string; user_id: string; roles: string[] }> {
    const response = await this.http.put<{ status: string; user_id: string; roles: string[] }>(`/auth/users/${id}/roles`, { roles });
    return response.data;
  }

  async changePassword(id: string, currentPassword: string, newPassword: string): Promise<{ status: string; user_id: string }> {
    const response = await this.http.put<{ status: string; user_id: string }>(`/auth/users/${id}/password`, {
      current_password: currentPassword,
      new_password: newPassword,
    });
    return response.data;
  }

  async listApiKeys(): Promise<{ data: ApiKey[]; pagination: { cursor: string | null; limit: number; has_more: boolean } }> {
    const response = await this.http.get<{ data: ApiKey[]; pagination: { cursor: string | null; limit: number; has_more: boolean } }>('/auth/api-keys');
    return response.data;
  }

  async createApiKey(input: {
    name: string;
    permissions?: string[];
    expires_at?: string;
  }): Promise<{ id: string; name: string; key: string; prefix: string; permissions: string[]; created_at: number; expires_at: number | null }> {
    const response = await this.http.post<{ id: string; name: string; key: string; prefix: string; permissions: string[]; created_at: number; expires_at: number | null }>('/auth/api-keys', input);
    return response.data;
  }

  async deleteApiKey(id: string): Promise<{ status: string; id: string }> {
    const response = await this.http.delete<{ status: string; id: string }>(`/auth/api-keys/${id}`);
    return response.data;
  }
}
