import { Injectable } from '@angular/core';
import { ApiService, PagedResult } from './api.service';
import { Observable } from 'rxjs';

export type NodeStatus = 'online' | 'offline' | 'maintenance';
export type Region = '华北' | '华东' | '华南' | '华中' | '西南' | '西北' | '东北';
export type Carrier = '电信' | '联通' | '移动' | '多线';

export interface Node {
  id: string;
  name: string;
  region: Region;
  datacenter_address: string;
  carrier: Carrier;
  capacity_gb: number;
  used_gb: number;
  status: NodeStatus;
  heartbeat_at: string;
}

export interface NodeWithStats {
  node: Node;
  resource_count: number;
}

export interface NodeResource {
  resource: any;
  hit_count: number;
  miss_count: number;
  hit_rate: number;
  last_accessed_at?: string;
}

export interface NodeQuery {
  page?: number;
  page_size?: number;
  region?: string;
  status?: string;
}

export const REGIONS: Region[] = ['华北', '华东', '华南', '华中', '西南', '西北', '东北'];
export const CARRIERS: Carrier[] = ['电信', '联通', '移动', '多线'];

@Injectable({ providedIn: 'root' })
export class NodeService {
  constructor(private api: ApiService) {}

  listNodes(query: NodeQuery): Observable<PagedResult<NodeWithStats>> {
    return this.api.get<PagedResult<NodeWithStats>>('/nodes', query);
  }

  getNode(id: string): Observable<NodeWithStats> {
    return this.api.get<NodeWithStats>(`/nodes/${id}`);
  }

  createNode(data: any): Observable<string> {
    return this.api.post<string>('/nodes', data);
  }

  updateNode(id: string, data: any): Observable<Node> {
    return this.api.put<Node>(`/nodes/${id}`, data);
  }

  deleteNode(id: string): Observable<boolean> {
    return this.api.delete<boolean>(`/nodes/${id}`);
  }

  updateNodeStatus(id: string, status: string, operator: string): Observable<boolean> {
    return this.api.put<boolean>(`/nodes/${id}/status`, { status, operator });
  }

  listNodeResources(id: string): Observable<{ items: NodeResource[]; total: number }> {
    return this.api.get<{ items: NodeResource[]; total: number }>(`/nodes/${id}/resources`);
  }
}
