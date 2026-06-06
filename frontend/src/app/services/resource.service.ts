import { Injectable } from '@angular/core';
import { ApiService, PagedResult } from './api.service';
import { Observable } from 'rxjs';

export interface Resource {
  id: string;
  original_filename: string;
  storage_path: string;
  mime_type: string;
  size_bytes: number;
  md5?: string;
  sha256: string;
  uploaded_at: string;
  uploaded_by: string;
  business_line: string;
  tags: string[];
  version: string;
  version_group: string;
}

export interface ResourceWithStats {
  resource: Resource;
  hit_count: number;
  miss_count: number;
  hit_rate: number;
  last_accessed_at?: string;
  published_nodes: string[];
}

export interface DirectoryNode {
  name: string;
  path: string;
  is_dir: boolean;
  children: DirectoryNode[];
  resource_id?: string;
}

export interface ResourceQuery {
  page?: number;
  page_size?: number;
  business_line?: string;
  tag?: string;
  search?: string;
  mime_type?: string;
  directory?: string;
}

@Injectable({ providedIn: 'root' })
export class ResourceService {
  constructor(private api: ApiService) {}

  listResources(query: ResourceQuery): Observable<PagedResult<ResourceWithStats>> {
    return this.api.get<PagedResult<ResourceWithStats>>('/resources', query);
  }

  getResource(id: string): Observable<ResourceWithStats> {
    return this.api.get<ResourceWithStats>(`/resources/${id}`);
  }

  createResource(data: any): Observable<string> {
    return this.api.post<string>('/resources', data);
  }

  updateResource(id: string, data: any): Observable<Resource> {
    return this.api.put<Resource>(`/resources/${id}`, data);
  }

  deleteResource(id: string): Observable<boolean> {
    return this.api.delete<boolean>(`/resources/${id}`);
  }

  listVersions(id: string): Observable<Resource[]> {
    return this.api.get<Resource[]>(`/resources/${id}/versions`);
  }

  publishResource(id: string, nodeIds: string[], operator: string): Observable<boolean> {
    return this.api.post<boolean>(`/resources/${id}/publish`, {
      node_ids: nodeIds,
      operator,
    });
  }

  unpublishResource(id: string, nodeIds: string[], operator: string): Observable<boolean> {
    return this.api.post<boolean>(`/resources/${id}/unpublish`, {
      node_ids: nodeIds,
      operator,
    });
  }

  getDirectoryTree(): Observable<DirectoryNode> {
    return this.api.get<DirectoryNode>('/resources/tree');
  }

  uploadResources(files: File[]): Observable<string[]> {
    return this.api.upload<string[]>('/resources/upload', files);
  }
}
