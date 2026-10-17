import { UIAbility, Want } from '@kit.AbilityKit'
import { window } from '@kit.ArkUI'
export default class EntryAbility extends UIAbility {
  onCreate(want: Want): void {
    AppStorage.setOrCreate('phase', (want.parameters?.phase as string) ?? 'create')
    const runId = (want.parameters?.runId as string) ?? 'trial-01'
    AppStorage.setOrCreate('runId', /^[a-zA-Z0-9_-]{1,48}$/.test(runId) ? runId : 'trial-01')
  }
  onWindowStageCreate(stage: window.WindowStage): void {
    stage.loadContent('pages/Index')
  }
}
