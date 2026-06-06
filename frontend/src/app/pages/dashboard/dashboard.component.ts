import { Component, OnInit } from '@angular/core';
import { StatsService, OverviewStats, HitRateEntry, TrendPoint } from '../../services/stats.service';
import { REGIONS } from '../../services/node.service';
import { EChartsOption } from 'echarts';

@Component({
  selector: 'app-dashboard',
  templateUrl: './dashboard.component.html',
})
export class DashboardComponent implements OnInit {
  overview?: OverviewStats;
  selectedRegion?: string;
  regions = ['全部', ...REGIONS];

  hitRateByNode: HitRateEntry[] = [];
  hitRateByTime: HitRateEntry[] = [];
  trendData: TrendPoint[] = [];

  trendChartOption: EChartsOption = {};
  hitRateChartOption: EChartsOption = {};
  capacityChartOption: EChartsOption = {};

  constructor(private statsService: StatsService) {}

  ngOnInit(): void {
    this.loadOverview();
    this.loadCharts();
  }

  loadOverview(): void {
    this.statsService.getOverview().subscribe({
      next: (data) => {
        this.overview = data;
        this.updateCapacityChart();
      },
    });
  }

  loadCharts(): void {
    const region = this.selectedRegion === '全部' ? undefined : this.selectedRegion;

    this.statsService.getHitRate({ group_by: 'node', region }).subscribe({
      next: (data) => {
        this.hitRateByNode = data;
        this.updateHitRateChart();
      },
    });

    this.statsService.getHitRate({ group_by: 'time_period', region }).subscribe({
      next: (data) => {
        this.hitRateByTime = data;
      },
    });

    this.statsService.getTrend({ days: 7, region }).subscribe({
      next: (data) => {
        this.trendData = data;
        this.updateTrendChart();
      },
    });
  }

  onRegionChange(region: string): void {
    this.selectedRegion = region;
    this.loadCharts();
  }

  updateTrendChart(): void {
    const dates = this.trendData.map((d) => {
      const date = new Date(d.timestamp);
      return `${date.getMonth() + 1}/${date.getDate()}`;
    });
    const requests = this.trendData.map((d) => d.requests);
    const hitRates = this.trendData.map((d) => (d.hit_rate * 100).toFixed(1));

    this.trendChartOption = {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'cross' },
      },
      legend: {
        data: ['请求量', '命中率'],
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: dates,
      },
      yAxis: [
        {
          type: 'value',
          name: '请求量',
          position: 'left',
        },
        {
          type: 'value',
          name: '命中率(%)',
          position: 'right',
          min: 0,
          max: 100,
          axisLabel: {
            formatter: '{value}%',
          },
        },
      ],
      series: [
        {
          name: '请求量',
          type: 'line',
          smooth: true,
          data: requests,
          areaStyle: {
            color: 'rgba(24, 144, 255, 0.1)',
          },
          lineStyle: {
            color: '#1890ff',
          },
          itemStyle: {
            color: '#1890ff',
          },
        },
        {
          name: '命中率',
          type: 'line',
          smooth: true,
          yAxisIndex: 1,
          data: hitRates,
          lineStyle: {
            color: '#52c41a',
          },
          itemStyle: {
            color: '#52c41a',
          },
        },
      ],
    };
  }

  updateHitRateChart(): void {
    const nodes = this.hitRateByNode.map((d) => d.key);
    const hitRates = this.hitRateByNode.map((d) => (d.hit_rate * 100).toFixed(1));

    this.hitRateChartOption = {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const data = params[0];
          return `${data.name}<br/>命中率: ${data.value}%`;
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        data: nodes,
        axisLabel: {
          rotate: 30,
        },
      },
      yAxis: {
        type: 'value',
        min: 0,
        max: 100,
        axisLabel: {
          formatter: '{value}%',
        },
      },
      series: [
        {
          type: 'bar',
          data: hitRates,
          itemStyle: {
            color: '#1890ff',
            borderRadius: [4, 4, 0, 0],
          },
          barWidth: '50%',
        },
      ],
    };
  }

  updateCapacityChart(): void {
    if (!this.overview) return;

    const nodes = this.overview.node_capacity_usage.map((d) => d[0]);
    const usage = this.overview.node_capacity_usage.map((d) => d[1].toFixed(1));

    this.capacityChartOption = {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const data = params[0];
          return `${data.name}<br/>使用率: ${data.value}%`;
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        data: nodes,
        axisLabel: {
          rotate: 30,
        },
      },
      yAxis: {
        type: 'value',
        min: 0,
        max: 100,
        axisLabel: {
          formatter: '{value}%',
        },
      },
      series: [
        {
          type: 'bar',
          data: usage,
          itemStyle: {
            color: '#fa8c16',
            borderRadius: [4, 4, 0, 0],
          },
          barWidth: '50%',
        },
      ],
    };
  }

  exportReport(): void {
    this.statsService.downloadCsv(7);
  }

  formatPercent(value: number): string {
    return (value * 100).toFixed(2) + '%';
  }

  getHitRateByKey(key: string): number {
    const entry = this.hitRateByTime.find((e) => e.key === key);
    return entry?.hit_rate || 0;
  }
}
