import { Component, OnInit } from '@angular/core';
import { NzModalService } from 'ng-zorro-antd/modal';
import { NzMessageService } from 'ng-zorro-antd/message';
import {
  PreheatService,
  PreheatTask,
} from '../../services/preheat.service';
import { ResourceService } from '../../services/resource.service';
import { NodeService } from '../../services/node.service';
import { interval, Subscription } from 'rxjs';

@Component({
  selector: 'app-preheat',
  templateUrl: './preheat.component.html',
})
export class PreheatComponent implements OnInit {
  tasks: PreheatTask[] = [];
  total = 0;
  page = 1;
  pageSize = 20;
  loading = false;
  selectedStatus = '';
  statusOptions = [
    { label: '全部', value: '' },
    { label: '等待中', value: 'pending' },
    { label: '进行中', value: 'running' },
    { label: '已完成', value: 'done' },
    { label: '部分完成', value: 'partial' },
    { label: '失败', value: 'failed' },
    { label: '已取消', value: 'cancelled' },
  ];

  createVisible = false;
  createForm: any = {
    resource_ids: [] as string[],
    node_ids: [] as string[],
    priority: 'medium',
  };

  resourceOptions: any[] = [];
  nodeOptions: any[] = [];

  private refreshSubscription?: Subscription;

  constructor(
    private preheatService: PreheatService,
    private resourceService: ResourceService,
    private nodeService: NodeService,
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
    this.preheatService
      .listTasks({
        page: this.page,
        page_size: this.pageSize,
        status: this.selectedStatus || undefined,
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
    this.resourceService.listResources({ page_size: 100 }).subscribe({
      next: (data) => {
        this.resourceOptions = data.items.map((item) => ({
          label: item.resource.original_filename,
          value: item.resource.id,
        }));
      },
    });

    this.nodeService.listNodes({ page_size: 100, status: 'online' }).subscribe({
      next: (data) => {
        this.nodeOptions = data.items.map((item) => ({
          label: `${item.node.name} (${item.node.region})`,
          value: item.node.id,
        }));
      },
    });
  }

  onStatusChange(status: string): void {
    this.selectedStatus = status;
    this.page = 1;
    this.loadTasks();
  }

  onPageChange(page: number): void {
    this.page = page;
    this.loadTasks();
  }

  getStatusColor(status: string): string {
    return this.preheatService.getStatusColor(status as any);
  }

  getStatusText(status: string): string {
    return this.preheatService.getStatusText(status as any);
  }

  getPriorityText(priority: string): string {
    return this.preheatService.getPriorityText(priority as any);
  }

  getPriorityColor(priority: string): string {
    return this.preheatService.getPriorityColor(priority as any);
  }

  getProgress(task: PreheatTask): number {
    if (task.total === 0) return 0;
    return Math.round(((task.done + task.failed) / task.total) * 100);
  }

  formatDuration(seconds: number): string {
    if (seconds < 60) return `${seconds} 秒`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟`;
    return `${(seconds / 3600).toFixed(1)} 小时`;
  }

  openCreate(): void {
    this.createForm = {
      resource_ids: [],
      node_ids: [],
      priority: 'medium',
    };
    this.createVisible = true;
  }

  doCreate(): void {
    if (this.createForm.resource_ids.length === 0) {
      this.message.error('请选择要预热的资源');
      return;
    }
    if (this.createForm.node_ids.length === 0) {
      this.message.error('请选择目标节点');
      return;
    }

    this.preheatService
      .createTask({
        resource_ids: this.createForm.resource_ids,
        node_ids: this.createForm.node_ids,
        priority: this.createForm.priority,
        created_by: 'admin',
      })
      .subscribe({
        next: () => {
          this.message.success('预热任务已创建');
          this.createVisible = false;
          this.loadTasks();
        },
        error: () => {
          this.message.error('创建失败');
        },
      });
  }

  closeCreate(): void {
    this.createVisible = false;
  }

  cancelTask(task: PreheatTask): void {
    this.modal.confirm({
      nzTitle: '取消任务',
      nzContent: '确定要取消这个预热任务吗？',
      nzOnOk: () => {
        this.preheatService.cancelTask(task.task_id).subscribe({
          next: () => {
            this.message.success('已取消');
            this.loadTasks();
          },
          error: () => {
            this.message.error('取消失败');
          },
        });
      },
    });
  }

  retryTask(task: PreheatTask): void {
    this.preheatService.retryTask(task.task_id).subscribe({
      next: () => {
        this.message.success('重试任务已创建');
        this.loadTasks();
      },
      error: () => {
        this.message.error('重试失败');
      },
    });
  }

  canCancel(task: PreheatTask): boolean {
    return task.status === 'pending' || task.status === 'running';
  }

  canRetry(task: PreheatTask): boolean {
    return (task.status === 'failed' || task.status === 'partial') && task.failed > 0;
  }
}
