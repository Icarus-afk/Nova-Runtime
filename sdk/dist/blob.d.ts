import type { HttpClient } from './client';
import type { BlobMetadata, BlobUploadInput, BlobDownloadOptions, BlobListEntry, BlobFilter, BlobMetrics, StorageTier, Connection, PaginationInput } from './types';
export declare class BlobClient {
    private http;
    constructor(http: HttpClient);
    upload(key: string, content: string, options?: Omit<BlobUploadInput, 'key' | 'content'>): Promise<BlobMetadata>;
    download(key: string, options?: BlobDownloadOptions): Promise<string>;
    del(key: string): Promise<boolean>;
    multiDel(keys: string[]): Promise<number>;
    list(prefix?: string, options?: {
        delimiter?: string;
        pagination?: PaginationInput;
        filter?: BlobFilter;
    }): Promise<Connection<BlobListEntry>>;
    info(key: string): Promise<BlobMetadata>;
    copy(source: string, destination: string): Promise<BlobMetadata>;
    move(source: string, destination: string): Promise<BlobMetadata>;
    setTier(key: string, tier: StorageTier): Promise<BlobMetadata>;
    setExpiry(key: string, expiresAt: Date): Promise<BlobMetadata>;
    removeExpiry(key: string): Promise<BlobMetadata>;
    getStats(): Promise<BlobMetrics>;
    listIterator(prefix?: string, options?: {
        delimiter?: string;
        filter?: BlobFilter;
    }): AsyncIterable<BlobListEntry>;
}
