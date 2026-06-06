import { Component, OnInit, ViewChild } from '@angular/core';
import { NzModalService } from 'ng-zorro-antd/modal';
import { NzMessageService } from 'ng-zorro-antd/message';
import { NzUploadChangeParam, NzUploadFile } from 'ng-zorro-antd/upload';
import {
  ResourceService,
  ResourceWithStats,
  DirectoryNode,
  Resource,
} from '../../services/resource.service';
import { NodeService } from '../../services/node.service';
import { NzFormatEmitEvent, NzTreeNode } from 'ng-zorro-antd/tree';
import { HttpResponse } from '@angular/common/http';

@Component({
  selector: 'app-resources',
  templateUrl: './resources.component.html',
})
export class ResourcesComponent implements OnInit {
  resources: ResourceWithStats[] = [];
  total = 0;
  page = 1;
  pageSize = 20;
  loading = false;
  searchText = '';
  selectedDirectory = '/';

  treeNodes: NzTreeNode[] = [];
  directoryTree?: DirectoryNode;

  uploadVisible = false;
  uploadFiles: NzUploadFile[] = [];
  uploadProgress: Record<string, number> = {};

  versionVisible = false;
  versions: Resource[] = [];
  currentResourceId = '';

  publishVisible = false;
  publishNodeIds: string[] = [];
  publishResourceId = '';
  nodes: any[] = [];

  constructor(
    private resourceService: ResourceService,
    private nodeService: NodeService,
    private modal: NzModalService,
    private message: NzMessageService
  ) {}

  ngOnInit(): void {
    this.loadResources();
    this.loadDirectoryTree();
    this.loadNodes();
  }

  loadResources(): void {
    this.loading = true;
    this.resourceService
      .listResources({
        page: this.page,
        page_size: this.pageSize,
        search: this.searchText || undefined,
        directory: this.selectedDirectory === '/' ? undefined : this.selectedDirectory,
      })
      .subscribe({
        next: (data) => {
          this.resources = data.items;
          this.total = data.total;
          this.loading = false;
        },
        error: () => {
          this.loading = false;
        },
      });
  }

  loadDirectoryTree(): void {
    this.resourceService.getDirectoryTree().subscribe({
      next: (data) => {
        this.directoryTree = data;
        this.treeNodes = this.buildTreeNodes(data);
      },
    });
  }

  buildTreeNodes(node: DirectoryNode): NzTreeNode[] {
    return node.children.map((child) => ({
      title: child.name,
      key: child.path,
      isLeaf: !child.is_dir,
      expanded: false,
      children: child.is_dir ? this.buildTreeNodes(child) : undefined,
    }));
  }

  onTreeNodeClick(event: NzFormatEmitEvent): void {
    const node = event.node;
    if (node?.isLeaf) {
      this.selectedDirectory = '';
      this.searchText = node.title as string;
    } else {
      this.selectedDirectory = node?.key as string;
    }
    this.page = 1;
    this.loadResources();
  }

  loadNodes(): void {
    this.nodeService.listNodes({ page_size: 100 }).subscribe({
      next: (data) => {
        this.nodes = data.items.map((item) => ({
          label: item.node.name,
          value: item.node.id,
        }));
      },
    });
  }

  onSearch(): void {
    this.page = 1;
    this.loadResources();
  }

  onPageChange(page: number): void {
    this.page = page;
    this.loadResources();
  }

  formatSize(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }

  formatPercent(value: number): string {
    return (value * 100).toFixed(2) + '%';
  }

  openUpload(): void {
    this.uploadFiles = [];
    this.uploadProgress = {};
    this.uploadVisible = true;
  }

  handleUploadChange(info: NzUploadChangeParam): void {
    const status = info.file.status;
    if (status === 'uploading') {
      this.uploadProgress[info.file.uid] = info.file.percent || 0;
    } else if (status === 'done') {
      this.message.success(`${info.file.name} 上传成功`);
      this.loadResources();
      this.loadDirectoryTree();
    } else if (status === 'error') {
      this.message.error(`${info.file.name} 上传失败`);
    }
  }

  beforeUpload = (file: NzUploadFile): boolean => {
    const maxSize = 100 * 1024 * 1024;
    if (file.size && file.size > maxSize) {
      this.message.error(`${file.name} 超过 100MB 限制`);
      return false;
    }
    return true;
  };

  customUpload = (item: any): void => {
    const formData = new FormData();
    formData.append('file', item.file as File);

    const xhr = new XMLHttpRequest();
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) {
        const percent = Math.round((e.loaded / e.total) * 100);
        item.onProgress({ percent }, item.file);
      }
    };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        item.onSuccess({}, item.file);
      } else {
        item.onError(new Error(xhr.statusText), item.file);
      }
    };
    xhr.open('POST', '/api/resources/upload');
    xhr.send(formData);
  };

  closeUpload(): void {
    this.uploadVisible = false;
  }

  viewVersions(resourceId: string): void {
    this.currentResourceId = resourceId;
    this.resourceService.listVersions(resourceId).subscribe({
      next: (data) => {
        this.versions = data;
        this.versionVisible = true;
      },
    });
  }

  closeVersions(): void {
    this.versionVisible = false;
  }

  openPublish(resourceId: string): void {
    this.publishResourceId = resourceId;
    this.publishNodeIds = [];
    this.publishVisible = true;
  }

  doPublish(): void {
    if (this.publishNodeIds.length === 0) {
      this.message.warning('请选择目标节点');
      return;
    }
    this.resourceService
      .publishResource(this.publishResourceId, this.publishNodeIds, 'admin')
      .subscribe({
        next: () => {
          this.message.success('发布成功');
          this.publishVisible = false;
          this.loadResources();
        },
        error: () => {
          this.message.error('发布失败');
        },
      });
  }

  closePublish(): void {
    this.publishVisible = false;
  }

  deleteResource(resource: ResourceWithStats): void {
    this.modal.confirm({
      nzTitle: '确认删除',
      nzContent: `确定要删除资源 "${resource.resource.original_filename}" 吗？`,
      nzOnOk: () => {
        this.resourceService.deleteResource(resource.resource.id).subscribe({
          next: () => {
            this.message.success('删除成功');
            this.loadResources();
            this.loadDirectoryTree();
          },
          error: () => {
            this.message.error('删除失败');
          },
        });
      },
    });
  }

  getMimeIcon(mime: string): string {
    if (mime.startsWith('image/')) return 'picture';
    if (mime.startsWith('video/')) return 'video-camera';
    if (mime.startsWith('audio/')) return 'sound';
    if (mime.includes('javascript')) return 'code';
    if (mime.includes('css')) return 'file-text';
    if (mime.includes('json')) return 'file-json';
    if (mime.startsWith('font/')) return 'font-size';
    return 'file';
  }
}
