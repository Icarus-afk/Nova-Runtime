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
    async refreshToken(refreshToken) {
        const response = await this.http.post('/auth/refresh', { refresh_token: refreshToken });
        return response.data;
    }
    async logout() {
        await this.http.post('/auth/logout');
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
    async deleteUser(id) {
        const response = await this.http.delete(`/auth/users/${id}`);
        return response.data;
    }
    async updateRoles(id, roles) {
        const response = await this.http.put(`/auth/users/${id}/roles`, { roles });
        return response.data;
    }
    async changePassword(id, currentPassword, newPassword) {
        const response = await this.http.put(`/auth/users/${id}/password`, {
            current_password: currentPassword,
            new_password: newPassword,
        });
        return response.data;
    }
    async listApiKeys() {
        const response = await this.http.get('/auth/api-keys');
        return response.data;
    }
    async createApiKey(input) {
        const response = await this.http.post('/auth/api-keys', input);
        return response.data;
    }
    async deleteApiKey(id) {
        const response = await this.http.delete(`/auth/api-keys/${id}`);
        return response.data;
    }
}
exports.AuthClient = AuthClient;
//# sourceMappingURL=auth.js.map