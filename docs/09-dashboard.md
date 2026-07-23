# 09. Dashboard

Nova Runtime includes a React-based web dashboard for monitoring and managing the runtime. The dashboard is built with Vite and React 18.

## 1. Overview

The dashboard provides a graphical interface for:

*   Monitoring runtime health and metrics.
*   Managing subsystems (SQL, cache, queues, scheduler, search, blobs).
*   Configuring the runtime.
*   Viewing logs and events.
*   Managing users and API keys.

## 2. Running the Dashboard

### Development Mode

1.  Navigate to the dashboard directory:
    ```bash
    cd dashboard
    ```
2.  Install dependencies:
    ```bash
    npm install
    ```
3.  Start the development server:
    ```bash
    npm run dev
    ```
    The dashboard will be available at `http://localhost:5173`.

### Production Build

1.  Build the dashboard:
    ```bash
    npm run build
    ```
    The optimized build will be output to `dashboard/dist/`.
2.  Serve the build (e.g., with Nginx or the Vite preview server):
    ```bash
    npm run preview
    ```

## 3. Dashboard Structure

### Pages

| Path | Page | Description |
| :--- | :--- | :---------- |
| `/` | Dashboard | Overview of runtime health, metrics, and subsystem status. |
| `/database` | Database | Manage SQL tables, run queries, and view schema. |
| `/cache` | Cache | View cache statistics, list keys, and manage entries. |
| `/queue` | Queue | List queues, view messages, and manage queue settings. |
| `/scheduler` | Scheduler | Manage scheduled jobs, view job status, and trigger jobs. |
| `/search` | Search | Manage search indexes, index documents, and run queries. |
| `/blob` | Blob Storage | List blobs, upload/download blobs, and view metadata. |
| `/auth` | Users & API Keys | Manage users, roles, and API keys. |
| `/config` | Configuration | View and edit the runtime configuration. |
| `/logs` | Logs | View runtime logs and events. |

### Authentication

The dashboard requires authentication. Users must log in with a valid username and password (managed via the Auth subsystem).

*   **Login Page:** `/login`
*   **Protected Routes:** All other routes require authentication.

## 4. Configuration

The dashboard connects to the Nova Runtime API at `http://127.0.0.1:8642` by default. To change the API endpoint:

1.  Edit `dashboard/src/api/client.ts`:
    ```typescript
    export const API_BASE_URL = 'http://your-nova-host:8642';
    ```
2.  Rebuild the dashboard.

## 5. API Client

The dashboard uses a custom API client (`dashboard/src/api/client.ts`) to interact with the Nova Runtime REST API. The client:

*   Handles authentication via JWT.
*   Supports GET, POST, PUT, and DELETE methods.
*   Automatically includes the Authorization header.

Example usage:

```typescript
import { apiClient } from './api/client';

// Fetch runtime health
const health = await apiClient.get('/health');

// Run a SQL query
const result = await apiClient.post('/api/v1/sql/query', {
  query: 'SELECT * FROM users'
});
```

## 6. Authentication Context

The dashboard uses React's Context API for authentication state management (`dashboard/src/components/AuthContext.tsx`). The context provides:

*   `isAuthenticated`: Boolean indicating if the user is logged in.
*   `login`: Function to log in with username/password.
*   `logout`: Function to log out.
*   `user`: Current user information.

## 7. Error Handling

The dashboard includes an `ErrorBoundary` component (`dashboard/src/components/ErrorBoundary.tsx`) to catch and display errors gracefully.

## 8. Styling

The dashboard uses CSS modules for styling. Global styles are defined in `dashboard/src/styles/global.css`.

## 9. Building and Deploying

### With Nova Runtime

The dashboard is designed to be served alongside Nova Runtime. To deploy:

1.  Build the dashboard:
    ```bash
    cd dashboard
    npm run build
    ```
2.  Copy the `dist/` directory to your web server or include it in your Nova Runtime deployment.

### Standalone

To deploy the dashboard standalone (e.g., with Nginx):

1.  Build the dashboard.
2.  Configure Nginx to serve the `dist/` directory:
    ```nginx
    server {
      listen 80;
      server_name dashboard.example.com;

      location / {
        root /path/to/dashboard/dist;
        try_files $uri /index.html;
      }

      location /api/ {
        proxy_pass http://localhost:8642/;
      }
    }
    ```
3.  Restart Nginx:
    ```bash
    systemctl restart nginx
    ```

## 10. Notes

*   The dashboard is a single-page application (SPA) and requires the Nova Runtime API to be accessible.
*   For production use, ensure the API is secured with TLS and proper authentication.
*   The dashboard does not have dedicated API routes; it uses the same REST API as the CLI and other clients.