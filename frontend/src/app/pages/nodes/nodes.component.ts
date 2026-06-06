import { Component, OnInit } from '@angular/core';
import { NzModalService } from 'ng-zorro-antd/modal';
import { NzMessageService } from 'ng-zorro-antd/message';
import {
  NodeService,
  NodeWithStats,
  REGIONS,
  CARRIERS,
  Node,
} from '../../services/node.service';

@Component({
  selector: 'app-nodes',
  templateUrl: './nodes.component.html',
})
export class NodesComponent implements OnInit {
  nodes: NodeWithStats[] = [];
  total = 0;
  page = 1;
  pageSize = 20;
  loading = false;
  selectedRegion?: string;
  selectedStatus?: string;

  regions = ['全部', ...REGIONS];
  allRegions = REGIONS;
  carriers = CARRIERS;
  statuses = [
    { label: '全部', value: '' },
    { label: '在线', value: 'online' },
    { label: '离线', value: 'offline' },
    { label: '维护', value: 'maintenance' },
  ];

  createVisible = false;
  createForm: any = {
    name: '',
    region: '华北',
    datacenter_address: '',
    carrier: '电信',
    capacity_gb: 100,
  };

  detailVisible = false;
  selectedNode?: NodeWithStats;
  nodeResources: any[] = [];

  constructor(
    private nodeService: NodeService,
    private modal: NzModalService,
    private message: NzMessageService
  ) {}

  ngOnInit(): void {
    this.loadNodes();
  }

  loadNodes(): void {
    this.loading = true;
    this.nodeService
      .listNodes({
        page: this.page,
        page_size: this.pageSize,
        region: this.selectedRegion === '全部' ? undefined : this.selectedRegion,
        status: this.selectedStatus || undefined,
      })
      .subscribe({
        next: (data) => {
          this.nodes = data.items;
          this.total = data.total;
          this.loading = false;
        },
        error: () => {
          this.loading = false;
        },
      });
  }

  onRegionChange(region: string): void {
    this.selectedRegion = region;
    this.page = 1;
    this.loadNodes();
  }

  onStatusChange(status: string): void {
    this.selectedStatus = status;
    this.page = 1;
    this.loadNodes();
  }

  onPageChange(page: number): void {
    this.page = page;
    this.loadNodes();
  }

  getStatusColor(status: string): string {
    const colors: Record<string, string> = {
      online: 'success',
      offline: 'default',
      maintenance: 'warning',
    };
    return colors[status] || 'default';
  }

  getStatusText(status: string): string {
    const texts: Record<string, string> = {
      online: '在线',
      offline: '离线',
      maintenance: '维护中',
    };
    return texts[status] || status;
  }

  getCapacityPercent(node: Node): number {
    if (node.capacity_gb === 0) return 0;
    return (node.used_gb / node.capacity_gb) * 100;
  }

  openCreate(): void {
    this.createForm = {
      name: '',
      region: '华北',
      datacenter_address: '',
      carrier: '电信',
      capacity_gb: 100,
    };
    this.createVisible = true;
  }

  doCreate(): void {
    if (!this.createForm.name.trim()) {
      this.message.error('请输入节点名称');
      return;
    }
    if (!this.createForm.datacenter_address.trim()) {
      this.message.error('请输入机房地址');
      return;
    }

    this.nodeService
      .createNode({
        ...this.createForm,
        operator: 'admin',
      })
      .subscribe({
        next: () => {
          this.message.success('创建成功');
          this.createVisible = false;
          this.loadNodes();
        },
        error: () => {
          this.message.error('创建失败');
        },
      });
  }

  closeCreate(): void {
    this.createVisible = false;
  }

  changeStatus(node: NodeWithStats, status: string): void {
    this.nodeService.updateNodeStatus(node.node.id, status, 'admin').subscribe({
      next: () => {
        this.message.success(`已切换为${this.getStatusText(status)}`);
        this.loadNodes();
      },
      error: () => {
        this.message.error('状态切换失败');
      },
    });
  }

  viewNode(node: NodeWithStats): void {
    this.selectedNode = node;
    this.nodeService.listNodeResources(node.node.id).subscribe({
      next: (data) => {
        this.nodeResources = data.items;
        this.detailVisible = true;
      },
    });
  }

  closeDetail(): void {
    this.detailVisible = false;
  }

  deleteNode(node: NodeWithStats): void {
    this.modal.confirm({
      nzTitle: '确认删除',
      nzContent: `确定要删除节点 "${node.node.name}" 吗？该节点上的所有资源发布关系也将被清除。`,
      nzOnOk: () => {
        this.nodeService.deleteNode(node.node.id).subscribe({
          next: () => {
            this.message.success('删除成功');
            this.loadNodes();
          },
          error: () => {
            this.message.error('删除失败');
          },
        });
      },
    });
  }

  formatSizeGB(gb: number): string {
    if (gb < 1024) return gb + ' GB';
    return (gb / 1024).toFixed(2) + ' TB';
  }
}
