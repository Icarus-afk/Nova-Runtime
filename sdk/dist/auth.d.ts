import type { HttpClient } from './client';
import type { AuthResult, User, ApiKey, ApiKeyFull, Role, Connection, PaginationInput } from './types';
export declare class AuthClient {
    private http;
    constructor(http: HttpClient);
    login(username: string, password: string): Promise<AuthResult>;
    register(input: {
        username: string;
        email: string;
        password: string;
        displayName: string;
    }): Promise<AuthResult>;
    refreshToken(refreshToken: string): Promise<AuthResult>;
    logout(): Promise<void>;
    me(): Promise<User>;
    listUsers(options?: {
        status?: string;
        role?: string;
        search?: string;
        pagination?: PaginationInput;
    }): Promise<Connection<User>>;
    getUser(id: string): Promise<User>;
    createUser(input: {
        username: string;
        email: string;
        password?: string;
        displayName?: string;
        roles?: string[];
    }): Promise<User>;
    updateUser(id: string, input: {
        displayName?: string;
        email?: string;
        metadata?: Record<string, unknown>;
    }): Promise<User>;
    deleteUser(id: string): Promise<void>;
    suspendUser(id: string, reason?: string): Promise<User>;
    activateUser(id: string): Promise<User>;
    listApiKeys(options?: PaginationInput): Promise<Connection<ApiKey>>;
    createApiKey(input: {
        name: string;
        permissions?: string[];
        roles?: string[];
        expiresAt?: Date;
    }): Promise<ApiKeyFull>;
    deleteApiKey(id: string): Promise<void>;
    listRoles(): Promise<Role[]>;
    createRole(input: {
        name: string;
        description: string;
        permissions: string[];
    }): Promise<Role>;
    deleteRole(name: string): Promise<void>;
    grantRole(userId: string, roleName: string): Promise<User>;
    revokeRole(userId: string, roleName: string): Promise<User>;
}
