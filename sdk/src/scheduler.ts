import type { HttpClient } from './client';
import type {
  Job, JobExecution, CreateJobInput, UpdateJobInput,
  SchedulerStats, Connection, PaginationInput
} from './types';

export class SchedulerClient {
  constructor(
    private http: HttpClient
  ) {}

  async listJobs(options?: {
    state?: string;
    type?: string;
    tags?: string[];
    pagination?: PaginationInput;
  }): Promise<Connection<Job>> {
    const response = await this.http.get<Connection<Job>>('/scheduler/jobs', {
      query: { ...(options?.pagination as Record<string, unknown> ?? {}), state: options?.state, type: options?.type, tags: options?.tags?.join(',') },
    });
    return response.data;
  }

  async getJob(id: string): Promise<Job> {
    const response = await this.http.get<Job>(`/scheduler/jobs/${id}`);
    return response.data;
  }

  async createJob(input: CreateJobInput): Promise<Job> {
    const response = await this.http.post<Job>('/scheduler/jobs', input);
    return response.data;
  }

  async updateJob(id: string, input: UpdateJobInput): Promise<Job> {
    const response = await this.http.patch<Job>(`/scheduler/jobs/${id}`, input);
    return response.data;
  }

  async deleteJob(id: string): Promise<void> {
    await this.http.delete(`/scheduler/jobs/${id}`);
  }

  async pauseJob(id: string): Promise<Job> {
    const response = await this.http.post<Job>(`/scheduler/jobs/${id}/pause`);
    return response.data;
  }

  async resumeJob(id: string): Promise<Job> {
    const response = await this.http.post<Job>(`/scheduler/jobs/${id}/resume`);
    return response.data;
  }

  async triggerJob(id: string, input?: unknown): Promise<JobExecution> {
    const response = await this.http.post<JobExecution>(`/scheduler/jobs/${id}/trigger`, { input });
    return response.data;
  }

  async getJobHistory(
    jobId: string,
    options?: {
      status?: string;
      since?: Date;
      until?: Date;
      pagination?: PaginationInput;
    }
  ): Promise<Connection<JobExecution>> {
    const response = await this.http.get<Connection<JobExecution>>(
      `/scheduler/jobs/${jobId}/history`, {
        query: {
          ...(options?.pagination as Record<string, unknown> ?? {}),
          status: options?.status,
          since: options?.since?.toISOString(),
          until: options?.until?.toISOString(),
        },
      }
    );
    return response.data;
  }

  async getExecution(executionId: string): Promise<JobExecution> {
    const response = await this.http.get<JobExecution>(`/scheduler/executions/${executionId}`);
    return response.data;
  }

  async cancelExecution(executionId: string): Promise<void> {
    await this.http.post(`/scheduler/executions/${executionId}/cancel`);
  }

  async retryExecution(executionId: string): Promise<JobExecution> {
    const response = await this.http.post<JobExecution>(`/scheduler/executions/${executionId}/retry`);
    return response.data;
  }

  async getStats(): Promise<SchedulerStats> {
    const response = await this.http.get<SchedulerStats>('/scheduler/stats');
    return response.data;
  }

  async *listJobsIterator(options?: {
    state?: string;
    type?: string;
    tags?: string[];
  }): AsyncIterable<Job> {
    let cursor: string | undefined;
    let hasMore = true;
    while (hasMore) {
      const page = await this.listJobs({ ...options, pagination: { first: 100, after: cursor } });
      for (const edge of page.edges) {
        yield edge.node;
      }
      hasMore = page.pageInfo.hasNextPage;
      cursor = page.pageInfo.endCursor ?? undefined;
    }
  }
}
