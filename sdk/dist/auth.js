"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AuthClient = void 0;
class AuthClient {
    constructor(http) {
        this.http = http;
    }
    async login(username, password) {
        const response = await this.http.post('/auth/login', { username, password });
        return response.data;
    }
    async register(input) {
        const response = await this.http.post('/auth/register', input);
        return response.data;
    }
    async refreshToken(refreshToken) {
        const response = await this.http.post('/auth/token/refresh', { refreshToken });
        return response.data;
    }
    async logout() {
        await this.http.post('/auth/logout');
    }
    async me() {
        const response = await this.http.get('/auth/me');
        return response.data;
    }
    async listUsers(options) {
        const response = await this.http.get('/auth/users', { query: options });
        return response.data;
    }
    async getUser(id) {
        const response = await this.http.get(`/auth/users/${id}`);
        return response.data;
    }
    async createUser(input) {
        const response = await this.http.post('/auth/users', input);
        return response.data;
    }
    async updateUser(id, input) {
        const response = await this.http.patch(`/auth/users/${id}`, input);
        return response.data;
    }
    async deleteUser(id) {
        await this.http.delete(`/auth/users/${id}`);
    }
    async suspendUser(id, reason) {
        const response = await this.http.post(`/auth/users/${id}/suspend`, { reason });
        return response.data;
    }
    async activateUser(id) {
        const response = await this.http.post(`/auth/users/${id}/activate`);
        return response.data;
    }
    async listApiKeys(options) {
        const response = await this.http.get('/auth/keys', { query: options });
        return response.data;
    }
    async createApiKey(input) {
        const response = await this.http.post('/auth/keys', input);
        return response.data;
    }
    async deleteApiKey(id) {
        await this.http.delete(`/auth/keys/${id}`);
    }
    async listRoles() {
        const response = await this.http.get('/auth/roles');
        return response.data;
    }
    async createRole(input) {
        const response = await this.http.post('/auth/roles', input);
        return response.data;
    }
    async deleteRole(name) {
        await this.http.delete(`/auth/roles/${encodeURIComponent(name)}`);
    }
    async grantRole(userId, roleName) {
        const response = await this.http.post(`/auth/users/${userId}/roles`, { role: roleName });
        return response.data;
    }
    async revokeRole(userId, roleName) {
        const response = await this.http.delete(`/auth/users/${userId}/roles/${encodeURIComponent(roleName)}`);
        return response.data;
    }
}
exports.AuthClient = AuthClient;
//# sourceMappingURL=auth.js.map