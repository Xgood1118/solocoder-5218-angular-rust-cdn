import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { DashboardComponent } from './pages/dashboard/dashboard.component';
import { ResourcesComponent } from './pages/resources/resources.component';
import { NodesComponent } from './pages/nodes/nodes.component';
import { PreheatComponent } from './pages/preheat/preheat.component';
import { PurgeComponent } from './pages/purge/purge.component';

const routes: Routes = [
  { path: '', redirectTo: '/dashboard', pathMatch: 'full' },
  { path: 'dashboard', component: DashboardComponent },
  { path: 'resources', component: ResourcesComponent },
  { path: 'nodes', component: NodesComponent },
  { path: 'preheat', component: PreheatComponent },
  { path: 'purge', component: PurgeComponent },
];

@NgModule({
  imports: [RouterModule.forRoot(routes)],
  exports: [RouterModule],
})
export class AppRoutingModule {}
