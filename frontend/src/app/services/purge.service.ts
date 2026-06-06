import { Injectable } from '@angular/core';
import { ApiService, PagedResult } from './api.service';
import { Observable } from 'rxjs';

export type PurgeType = 'by_node' | 'by_resource' | 'by_time';
export type TaskStatus = 'pending' | 'running' | 'done' | 'partial' | 'failed' | 'cancelled';

export interface PurgeTask {
  task_id: string;
  purge_type: PurgeType;
  node_ids: string[];
  resource_ids: string[];
  days_not_accessed?: number;
  mime_types: string[];
  status: TaskStatus;
  total: number;
  done: number;
  failed: number;
  started_at?: string;
  finished_at?: string;
  created_by: string;
  dry_run: boolean;
  created_at: string;
}

export interface PurgeTaskQuery {
  page?: number;
  page_size?: number;
  status?: string;
  purge_type?: string;
}

export interface CreatePurgeRequest {
  purge_type: string;
  node_ids?: string[];
  resource_ids?: string[];
  days_not_accessed?: number;
  mime_types?: string[];
  created_by: string;
  dry_run?: boolean;
}

export interface PurgeResourceInfo {
  resource_id: string;
  resource_name: string;
  node_id: string;
  node_name: string;
  size_bytes: number;
  last_accessed?: string;
}

export interface DryRunResult {
  total_resources: number;
  resources: PurgeResourceInfo[];
  estimated_savings_gb: number;
}

@Injectable({ providedIn: 'root' })
export class PurgeService {
  constructor(private api: ApiService) {}

  listTasks(query: PurgeTaskQuery): Observable<PagedResult<PurgeTask>> {
    return this.api.get<PagedResult<PurgeTask>>('/purge', query);
  }

  getTask(taskId: string): Observable<PurgeTask> {
    return this.api.get<PurgeTask>(`/purge/${taskId}`);
  }

  createTask(request: CreatePurgeRequest): Observable<string> {
    return this.api.post<string>('/purge', request);
  }

  dryRun(request: CreatePurgeRequest): Observable<DryRunResult> {
    return this.api.post<DryRunResult>('/purge/dry-run', request);
  }

  getStatusColor(status: TaskStatus): string {
    const colors: Record<TaskStatus, string> = {
      pending: 'default',
      running: 'processing',
      done: 'success',
      partial: 'warning',
      failed: 'error',
      cancelled: 'default',
    };
    return colors[status];
  }

  getStatusText(status: TaskStatus): string {
    const texts: Record<TaskStatus, string> = {
      pending: '等待中',
      running: '进行中',
      done: '已完成',
      partial: '部分完成',
      failed: '失败',
      cancelled: '已取消',
    };
    return texts[status];
  }

  getTypeText(type: PurgeType): string {
    const texts: Record<PurgeType, string> = {
      by_node: '按节点',
      by_resource: '按资源',
      by_time: '按时间',
    };
    return texts[type];
  }
}
