import { Injectable } from '@angular/core';
import { ApiService, PagedResult } from './api.service';
import { Observable } from 'rxjs';

export type TaskStatus = 'pending' | 'running' | 'done' | 'partial' | 'failed' | 'cancelled';
export type TaskPriority = 'low' | 'medium' | 'high';

export interface PreheatTask {
  task_id: string;
  resource_ids: string[];
  node_ids: string[];
  status: TaskStatus;
  priority: TaskPriority;
  total: number;
  done: number;
  failed: number;
  failed_resources: string[];
  started_at?: string;
  finished_at?: string;
  created_by: string;
  estimated_duration_secs: number;
  created_at: string;
}

export interface PreheatTaskQuery {
  page?: number;
  page_size?: number;
  status?: string;
}

export interface CreatePreheatRequest {
  resource_ids: string[];
  node_ids: string[];
  priority?: string;
  created_by: string;
}

@Injectable({ providedIn: 'root' })
export class PreheatService {
  constructor(private api: ApiService) {}

  listTasks(query: PreheatTaskQuery): Observable<PagedResult<PreheatTask>> {
    return this.api.get<PagedResult<PreheatTask>>('/preheat', query);
  }

  getTask(taskId: string): Observable<PreheatTask> {
    return this.api.get<PreheatTask>(`/preheat/${taskId}`);
  }

  createTask(request: CreatePreheatRequest): Observable<string> {
    return this.api.post<string>('/preheat', request);
  }

  cancelTask(taskId: string): Observable<boolean> {
    return this.api.post<boolean>(`/preheat/${taskId}/cancel`, {});
  }

  retryTask(taskId: string): Observable<string> {
    return this.api.post<string>(`/preheat/${taskId}/retry`, {});
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

  getPriorityText(priority: TaskPriority): string {
    const texts: Record<TaskPriority, string> = {
      low: '低',
      medium: '中',
      high: '高',
    };
    return texts[priority];
  }

  getPriorityColor(priority: TaskPriority): string {
    const colors: Record<TaskPriority, string> = {
      low: 'default',
      medium: 'blue',
      high: 'red',
    };
    return colors[priority];
  }
}
