import { Component, OnInit } from '@angular/core';
import { NzModalService } from 'ng-zorro-antd/modal';
import { NzMessageService } from 'ng-zorro-antd/message';
import {
  PurgeService,
  PurgeTask,
  DryRunResult,
} from '../../services/purge.service';
import { NodeService, REGIONS } from '../../services/node.service';
import { ResourceService } from '../../services/resource.service';
import { interval, Subscription } from 'rxjs';

@Component({
  selector: 'app-purge',
  templateUrl: './purge.component.html',
})
export class PurgeComponent implements OnInit {
  tasks: PurgeTask[] = [];
  total = 0;
  page = 1;
  pageSize = 20;
  loading = false;
  selectedStatus = '';
  selectedType = '';

  statusOptions = [
    { label: '全部状态', value: '' },
    { label: '等待中', value: 'pending' },
    { label: '进行中', value: 'running' },
    { label: '已完成', value: 'done' },
    { label: '失败', value: 'failed' },
  ];

  typeOptions = [
    { label: '全部类型', value: '' },
    { label: '按节点', value: 'by_node' },
    { label: '按资源', value: 'by_resource' },
    { label: '按时间', value: 'by_time' },
  ];

  createVisible = false;
  createForm: any = {
    purge_type: 'by_node',
    node_ids: [],
    resource_ids: [],
    days_not_accessed: 7,
    mime_types: [] as string[],
  };

  dryRunResult?: DryRunResult;
  dryRunVisible = false;
  isDryRunLoading = false;

  nodeOptions: any[] = [];
  resourceOptions: any[] = [];

  mimeOptions = [
    { label: '图片', value: 'image/' },
    { label: '视频', value: 'video/' },
    { label: '音频', value: 'audio/' },
    { label: 'JS', value: 'application/javascript' },
    { label: 'CSS', value: 'text/css' },
    { label: 'JSON', value: 'application/json' },
    { label: '字体', value: 'font/' },
  ];

  private refreshSubscription?: Subscription;

  constructor(
    private purgeService: PurgeService,
    private nodeService: NodeService,
    private resourceService: ResourceService,
    private modal: NzModalService,
    private message: NzMessageService
  ) {}

  ngOnInit(): void {
    this.loadTasks();
    this.loadOptions();

    this.refreshSubscription = interval(5000).subscribe(() => {
      const hasRunningTasks = this.tasks.some(
        (t) => t.status === 'running' || t.status === 'pending'
      );
      if (hasRunningTasks) {
        this.loadTasks();
      }
    });
  }

  ngOnDestroy(): void {
    this.refreshSubscription?.unsubscribe();
  }

  loadTasks(): void {
    this.loading = true;
    this.purgeService
      .listTasks({
        page: this.page,
        page_size: this.pageSize,
        status: this.selectedStatus || undefined,
        purge_type: this.selectedType || undefined,
      })
      .subscribe({
        next: (data) => {
          this.tasks = data.items;
          this.total = data.total;
          this.loading = false;
        },
        error: () => {
          this.loading = false;
        },
      });
  }

  loadOptions(): void {
    this.nodeService.listNodes({ page_size: 100 }).subscribe({
      next: (data) => {
        this.nodeOptions = data.items.map((item) => ({
          label: `${item.node.name} (${item.node.region})`,
          value: item.node.id,
        }));
      },
    });

    this.resourceService.listResources({ page_size: 100 }).subscribe({
      next: (data) => {
        this.resourceOptions = data.items.map((item) => ({
          label: item.resource.original_filename,
          value: item.resource.id,
        }));
      },
    });
  }

  onStatusChange(status: string): void {
    this.selectedStatus = status;
    this.page = 1;
    this.loadTasks();
  }

  onTypeChange(type: string): void {
    this.selectedType = type;
    this.page = 1;
    this.loadTasks();
  }

  onPageChange(page: number): void {
    this.page = page;
    this.loadTasks();
  }

  getStatusColor(status: string): string {
    return this.purgeService.getStatusColor(status as any);
  }

  getStatusText(status: string): string {
    return this.purgeService.getStatusText(status as any);
  }

  getTypeText(type: string): string {
    return this.purgeService.getTypeText(type as any);
  }

  getTypeIcon(type: string): string {
    const icons: Record<string, string> = {
      by_node: 'cloud-server',
      by_resource: 'file',
      by_time: 'clock-circle',
    };
    return icons[type] || 'file';
  }

  openCreate(): void {
    this.createForm = {
      purge_type: 'by_node',
      node_ids: [],
      resource_ids: [],
      days_not_accessed: 7,
      mime_types: [],
    };
    this.dryRunResult = undefined;
    this.createVisible = true;
  }

  doDryRun(): void {
    if (!this.validateForm()) return;

    this.isDryRunLoading = true;
    this.purgeService
      .dryRun({
        purge_type: this.createForm.purge_type,
        node_ids: this.createForm.node_ids.length > 0 ? this.createForm.node_ids : undefined,
        resource_ids: this.createForm.resource_ids.length > 0 ? this.createForm.resource_ids : undefined,
        days_not_accessed: this.createForm.purge_type === 'by_time' ? this.createForm.days_not_accessed : undefined,
        mime_types: this.createForm.mime_types.length > 0 ? this.createForm.mime_types : undefined,
        created_by: 'admin',
      })
      .subscribe({
        next: (data) => {
          this.dryRunResult = data;
          this.dryRunVisible = true;
          this.isDryRunLoading = false;
        },
        error: () => {
          this.message.error('试运行失败');
          this.isDryRunLoading = false;
        },
      });
  }

  doCreate(): void {
    if (!this.validateForm()) return;

    this.purgeService
      .createTask({
        purge_type: this.createForm.purge_type,
        node_ids: this.createForm.node_ids.length > 0 ? this.createForm.node_ids : undefined,
        resource_ids: this.createForm.resource_ids.length > 0 ? this.createForm.resource_ids : undefined,
        days_not_accessed: this.createForm.purge_type === 'by_time' ? this.createForm.days_not_accessed : undefined,
        mime_types: this.createForm.mime_types.length > 0 ? this.createForm.mime_types : undefined,
        created_by: 'admin',
        dry_run: false,
      })
      .subscribe({
        next: () => {
          this.message.success('清理任务已创建');
          this.createVisible = false;
          this.loadTasks();
        },
        error: () => {
          this.message.error('创建失败');
        },
      });
  }

  validateForm(): boolean {
    if (this.createForm.purge_type === 'by_node' && this.createForm.node_ids.length === 0) {
      this.message.error('请选择要清理的节点');
      return false;
    }
    if (this.createForm.purge_type === 'by_resource' && this.createForm.resource_ids.length === 0) {
      this.message.error('请选择要清理的资源');
      return false;
    }
    if (this.createForm.purge_type === 'by_time') {
      if (this.createForm.node_ids.length === 0) {
        this.message.error('请选择目标节点');
        return false;
      }
      if (!this.createForm.days_not_accessed || this.createForm.days_not_accessed < 1) {
        this.message.error('请设置未访问天数');
        return false;
      }
    }
    return true;
  }

  closeCreate(): void {
    this.createVisible = false;
  }

  closeDryRun(): void {
    this.dryRunVisible = false;
  }

  confirmFromDryRun(): void {
    this.dryRunVisible = false;
    this.doCreate();
  }

  formatSize(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }

  getProgress(task: PurgeTask): number {
    if (task.total === 0) return 0;
    return Math.round((task.done / task.total) * 100);
  }
}
