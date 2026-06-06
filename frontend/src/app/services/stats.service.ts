import { Injectable } from '@angular/core';
import { ApiService } from './api.service';
import { Observable } from 'rxjs';

export interface OverviewStats {
  total_resources: number;
  total_nodes: number;
  overall_hit_rate: number;
  total_requests_today: number;
  total_hits_today: number;
  top_hot_resources: [string, number][];
  top_cold_resources: [string, number][];
  node_capacity_usage: [string, number][];
  active_preheat_tasks: number;
  active_purge_tasks: number;
}

export interface HitRateEntry {
  key: string;
  hit_count: number;
  miss_count: number;
  hit_rate: number;
}

export interface TrendPoint {
  timestamp: string;
  requests: number;
  hits: number;
  hit_rate: number;
}

export interface HitRateQuery {
  group_by: string;
  region?: string;
  time_period?: string;
}

export interface TrendQuery {
  days?: number;
  region?: string;
  node_id?: string;
}

@Injectable({ providedIn: 'root' })
export class StatsService {
  constructor(private api: ApiService) {}

  getOverview(): Observable<OverviewStats> {
    return this.api.get<OverviewStats>('/stats/overview');
  }

  getHitRate(query: HitRateQuery): Observable<HitRateEntry[]> {
    return this.api.get<HitRateEntry[]>('/stats/hit-rate', query);
  }

  getTrend(query: TrendQuery): Observable<TrendPoint[]> {
    return this.api.get<TrendPoint[]>('/stats/trend', query);
  }

  exportCsv(days: number = 7): Observable<Blob> {
    return this.api.get<any>('/stats/export', { days });
  }

  downloadCsv(days: number = 7): void {
    const link = document.createElement('a');
    link.href = `/api/stats/export?days=${days}`;
    link.download = `cdn_stats_${days}days.csv`;
    link.click();
  }
}
