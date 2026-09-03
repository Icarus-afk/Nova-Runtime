import type { HttpClient } from './client';
import type { AuthResult, User, ApiKey, PaginationInput } from './types';
export declare class AuthClient {
    private http;
    constructor(http: HttpClient);
    login(username: string, password: string): Promise<AuthResult>;
    refreshToken(refreshToken: string): Promise<AuthResult>;
    logout(): Promise<void>;
    listUsers(options?: PaginationInput): Promise<{
        data: User[];
        pagination: {
            cursor: string | null;
            limit: number;
            has_more: boolean;
        };
    }>;
    getUser(id: string): Promise<User>;
    createUser(input: {
        username: string;
        password: string;
        roles?: string[];
    }): Promise<{
        id: string;
        username: string;
        roles: string[];
        status: string;
    }>;
    deleteUser(id: string): Promise<{
        status: string;
        id: string;
    }>;
    updateRoles(id: string, roles: string[]): Promise<{
        status: string;
        user_id: string;
        roles: string[];
    }>;
    changePassword(id: string, currentPassword: string, newPassword: string): Promise<{
        status: string;
        user_id: string;
    }>;
    listApiKeys(): Promise<{
        data: ApiKey[];
        pagination: {
            cursor: string | null;
            limit: number;
            has_more: boolean;
        };
    }>;
    createApiKey(input: {
        name: string;
        permissions?: string[];
        expires_at?: string;
    }): Promise<{
        id: string;
        name: string;
        key: string;
        prefix: string;
        permissions: string[];
        created_at: number;
        expires_at: number | null;
    }>;
    deleteApiKey(id: string): Promise<{
        status: string;
        id: string;
    }>;
}
