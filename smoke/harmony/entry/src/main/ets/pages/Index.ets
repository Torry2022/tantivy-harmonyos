import { common } from '@kit.AbilityKit'
import probe from 'libtantivy_probe.so'
@Entry
@Component
struct Index {
  @State result: string = 'Ready'
  aboutToAppear(): void {
    const context = getContext(this) as common.UIAbilityContext
    const phase: string = AppStorage.get<string>('phase') ?? 'create'
    const runId: string = AppStorage.get<string>('runId') ?? 'trial-01'
    this.result = probe.run(context.filesDir + '/' + runId, phase)
  }
  build() {
    Column({ space: 20 }) {
      Text('Tantivy validation').fontSize(24)
      Text(this.result).fontSize(16)
    }.padding(24).width('100%').height('100%').justifyContent(FlexAlign.Center)
  }
}
