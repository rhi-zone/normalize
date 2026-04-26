import { EventEmitter } from 'events';
import * as path from 'path';

export interface Logger {
    log(message: string): void;
    error(message: string): void;
}

// Logs to a file
@Injectable()
export class FileLogger implements Logger {
    private prefix: string;

    constructor(prefix: string) {
        this.prefix = prefix;
    }

    log(message: string): void {
        console.log(`[${this.prefix}] ${message}`);
    }

    error(message: string): void {
        console.error(`[${this.prefix}] ERROR: ${message}`);
    }
}

export function formatPath(filePath: string): string {
    const normalized = path.normalize(filePath);
    if (normalized.startsWith('/')) {
        return normalized;
    }
    return `./${normalized}`;
}

export function groupBy<T>(items: T[], key: (item: T) => string): Map<string, T[]> {
    const result = new Map<string, T[]>();
    for (const item of items) {
        const k = key(item);
        const group = result.get(k) ?? [];
        group.push(item);
        result.set(k, group);
    }
    return result;
}
