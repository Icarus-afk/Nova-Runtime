import type { HttpClient } from './client';
import type { Job, JobExecution, CreateJobInput, UpdateJobInput, SchedulerStats, Connection, PaginationInput } from './types';
export declare class SchedulerClient {
    private http;
    constructor(http: HttpClient);
    listJobs(options?: {
        state?: string;
        type?: string;
        tags?: string[];
        pagination?: PaginationInput;
    }): Promise<Connection<Job>>;
    getJob(id: string): Promise<Job>;
    createJob(input: CreateJobInput): Promise<Job>;
    updateJob(id: string, input: UpdateJobInput): Promise<Job>;
    deleteJob(id: string): Promise<void>;
    pauseJob(id: string): Promise<Job>;
    resumeJob(id: string): Promise<Job>;
    triggerJob(id: string, input?: unknown): Promise<JobExecution>;
    getJobHistory(jobId: string, options?: {
        status?: string;
        since?: Date;
        until?: Date;
        pagination?: PaginationInput;
    }): Promise<Connection<JobExecution>>;
    getExecution(executionId: string): Promise<JobExecution>;
    cancelExecution(executionId: string): Promise<void>;
    retryExecution(executionId: string): Promise<JobExecution>;
    getStats(): Promise<SchedulerStats>;
    listJobsIterator(options?: {
        state?: string;
        type?: string;
        tags?: string[];
    }): AsyncIterable<Job>;
}
